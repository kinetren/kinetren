#include "bc.h"

#include <QPainter>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <deque>
#include <climits>
#include <limits>
#include <map>
#include <mutex>
#include <string>
#include <system_error>
#include <thread>
#include <utility>
#include <vector>

#include <QSize>

#include <avcpp/audioresampler.h>
#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/dictionary.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/frame.h>
#include <avcpp/videorescaler.h>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/mathematics.h>
#include <libavutil/pixdesc.h>
}
#include "bc_internal.h"

namespace burstcon {
namespace {

/// Hot ring inside each `AvcppMediaDecoderBackend` (below ConCache). ConCache `maxVideoBytes` can be
/// much larger, but this cap still bounds RAM while one file is being decoded sequentially.
constexpr size_t kDecoderVideoFrameCacheMaxFrames = 384*384;
constexpr uint64_t kDecoderVideoFrameCacheMaxBytes = 1024ULL * 1024ULL * 1024ULL;
const av::Rational kMillisecondsTimebase{1, 1000};


int64_t qualityArea(const BurstQuality &q)
{
    if (q.width <= 0 || q.height <= 0) {
        return 0;
    }
    return static_cast<int64_t>(q.width) * static_cast<int64_t>(q.height);
}

qint64 timestampToMs(const av::Timestamp &ts)
{
    if (ts.isNoPts()) {
        return -1;
    }
    return ts.timestamp(kMillisecondsTimebase);
}

/// VP9/AV1 in ISOBMFF often use many small packets (and more decoder delay) per displayed frame; the
/// res×pixel budget that works for H.26x can run out with passStop=videoPacketLimit and decNull high.
static int videoPacketBudgetCodecMul(AVCodecID id)
{
    switch (id) {
    case AV_CODEC_ID_VP9:
    case AV_CODEC_ID_AV1:
        return 12;
    case AV_CODEC_ID_HEVC:
        // Long reorder / multi-layer NALU bursts; 4K + prefetch benefits from a higher cap.
        return 10;
    case AV_CODEC_ID_VP8:
        return 2;
    default:
        return 1;
    }
}

uint64_t imageBytes(const QImage &img)
{
#if QT_VERSION >= QT_VERSION_CHECK(5, 10, 0)
    return static_cast<uint64_t>(img.sizeInBytes());
#else
    return static_cast<uint64_t>(img.byteCount());
#endif
}

bool avPixelFormatHasAlpha(AVPixelFormat fmt)
{
    const AVPixFmtDescriptor *desc = av_pix_fmt_desc_get(fmt);
    return desc != nullptr && (desc->flags & AV_PIX_FMT_FLAG_ALPHA) != 0;
}

qint64 packetTimestampToMs(const av::Packet &pkt)
{
    qint64 tsMs = timestampToMs(pkt.dts());
    if (tsMs >= 0) {
        return tsMs;
    }
    return timestampToMs(pkt.pts());
}

struct PacketIndexEntry {
    BurstIndex index = 0;
    int64_t bytePos = -1;
};

PacketIndexEntry makePacketIndexEntry(const av::Packet &pkt)
{
    PacketIndexEntry entry;
    entry.bytePos = pkt.raw() ? pkt.raw()->pos : -1;
    const qint64 tsMs = packetTimestampToMs(pkt);
    if (tsMs >= 0) {
        entry.index = msToBurstIndex(tsMs);
    }
    return entry;
}

void dedupePacketIndex(std::vector<PacketIndexEntry> &entries)
{
    std::sort(entries.begin(), entries.end(), [](const PacketIndexEntry &a, const PacketIndexEntry &b) {
        if (a.index != b.index) {
            return a.index < b.index;
        }
        return a.bytePos < b.bytePos;
    });
    entries.erase(std::unique(entries.begin(), entries.end(), [](const PacketIndexEntry &a, const PacketIndexEntry &b) {
        return a.index == b.index && a.bytePos == b.bytePos;
    }), entries.end());
}

} // namespace

/// Shared probe pass: stream metadata + sparse audio byte index for coarse `seekAudioLocked`.
/// Video random access uses `av::FormatContext::seek` (`av_seek_frame`) with `AVSEEK_FLAG_BACKWARD`.
struct SharedMediaFile {
    QString path;
    std::mutex mutex;
    bool indexed = false;
    BurstOriginInfo origin;
    int videoStream = -1;
    int audioStream = -1;
    BurstIndex videoFrameStep = std::max<BurstIndex>(1, kBurstTimebaseHz / 30);
    std::vector<PacketIndexEntry> audioPackets;

    void ensureIndexed()
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (indexed) {
            return;
        }
        using namespace av;
        FormatContext input;
        std::error_code ec;
        gAvcppOpenCalls.fetch_add(1);
        input.openInput(path.toStdString(), ec);
        if (ec) {
            return;
        }
        input.findStreamInfo(ec);
        if (ec) {
            return;
        }
        for (size_t i = 0; i < input.streamsCount(); ++i) {
            auto st = input.stream(i);
            if (videoStream < 0 && st.mediaType() == AVMEDIA_TYPE_VIDEO) {
                videoStream = static_cast<int>(i);
                const double fps = st.averageFrameRate().getDouble();
                if (fps > 1.0) {
                    videoFrameStep = std::max<BurstIndex>(1, static_cast<BurstIndex>(kBurstTimebaseHz / fps));
                }
                if (auto *codecpar = st.codecParameters().raw()) {
                    origin.sourceVideoQuality = BurstQuality{codecpar->width, codecpar->height};
                }
            } else if (audioStream < 0 && st.mediaType() == AVMEDIA_TYPE_AUDIO) {
                audioStream = static_cast<int>(i);
                if (auto *codecpar = st.codecParameters().raw()) {
                    origin.sourceAudioSampleRate = std::max(8000, codecpar->sample_rate);
#if API_NEW_CHANNEL_LAYOUT
                    origin.sourceAudioChannels = std::max(1, codecpar->ch_layout.nb_channels);
#else
                    origin.sourceAudioChannels = std::max(1, codecpar->channels);
#endif
                }
            }
        }
        if (audioStream >= 0) {
            qint64 lastAudioIndexedMs = std::numeric_limits<qint64>::min();
            constexpr qint64 kAudioIndexMinIntervalMs = 400;
            while (true) {
                Packet pkt = input.readPacket(ec);
                gAvcppPacketReads.fetch_add(1);
                if (ec || !pkt) {
                    ec.clear();
                    break;
                }
                if (pkt.streamIndex() != audioStream) {
                    continue;
                }
                const qint64 pktMs = packetTimestampToMs(pkt);
                bool record = audioPackets.empty();
                if (!record && pktMs >= 0) {
                    record = (pktMs - lastAudioIndexedMs) >= kAudioIndexMinIntervalMs;
                }
                if (!record && pktMs < 0) {
                    record = audioPackets.size() < 8;
                }
                if (record) {
                    PacketIndexEntry entry = makePacketIndexEntry(pkt);
                    if (entry.bytePos >= 0) {
                        audioPackets.push_back(entry);
                        if (pktMs >= 0) {
                            lastAudioIndexedMs = pktMs;
                        }
                    }
                }
            }
        }
        if (audioPackets.empty()) {
            audioPackets.push_back(PacketIndexEntry{0, 0});
        }
        dedupePacketIndex(audioPackets);
        indexed = true;
    }

    PacketIndexEntry audioStartFor(BurstIndex target) const
    {
        if (audioPackets.empty()) {
            return PacketIndexEntry{0, 0};
        }
        auto it = std::upper_bound(audioPackets.begin(), audioPackets.end(), target,
                                   [](BurstIndex value, const PacketIndexEntry &entry) {
                                       return value < entry.index;
                                   });
        if (it == audioPackets.begin()) {
            return *it;
        }
        --it;
        return *it;
    }
};

namespace {

struct DecodedVideoFrameKey {
    BurstIndex index = 0;
    int rgbW = 0;
    int rgbH = 0;
    friend bool operator<(const DecodedVideoFrameKey &a, const DecodedVideoFrameKey &b)
    {
        if (a.index != b.index) {
            return a.index < b.index;
        }
        if (a.rgbW != b.rgbW) {
            return a.rgbW < b.rgbW;
        }
        return a.rgbH < b.rgbH;
    }
};

static QSize videoRgbSizeForQuality(int srcW, int srcH, const BurstQuality &q)
{
    if (srcW <= 0 || srcH <= 0) {
        return QSize();
    }
    if (q.width <= 0 || q.height <= 0) {
        return QSize(srcW, srcH);
    }
    const QSize srcSize(srcW, srcH);
    const QSize scaled = srcSize.scaled(q.width, q.height, Qt::KeepAspectRatio);
    return scaled.isValid() && scaled.width() > 0 && scaled.height() > 0 ? scaled : srcSize;
}

class AvcppMediaDecoderBackend : public IMediaDecoderBackend
{
public:
    explicit AvcppMediaDecoderBackend(std::shared_ptr<SharedMediaFile> sharedFile,
                                      bool skipFrames,
                                      bool wantVideo,
                                      bool wantAudio)
        : sharedFile(std::move(sharedFile))
        , skipFrames(skipFrames)
        , wantVideo(wantVideo)
        , wantAudio(wantAudio)
    {
    }

    bool load(const QString &path) override
    {
        using namespace av;
        std::lock_guard<std::mutex> videoLock(videoMutex);
        std::lock_guard<std::mutex> audioLock(audioMutex);
        if (sharedFile) {
            sharedFile->path = path;
            sharedFile->ensureIndexed();
        }
        this->path = path;
        loaded = false;
        origin = sharedFile ? sharedFile->origin : BurstOriginInfo{};
        videoStream = sharedFile ? sharedFile->videoStream : -1;
        audioStream = sharedFile ? sharedFile->audioStream : -1;
        videoFrameStep = sharedFile ? sharedFile->videoFrameStep : std::max<BurstIndex>(1, kBurstTimebaseHz / 30);
        videoCursorIndex = -1;
        audioCursorIndex = -1;
        videoFrameCache.clear();
        videoFrameBytesUsed = 0;
        audioBurstCache.clear();
        audioPending.clear();
        videoDec.reset();
        audioDec.reset();
        videoRescaler.reset();
        audioResampler.reset();
        scalerSrcW = 0;
        scalerSrcH = 0;
        scalerOutputsAlpha = false;
        videoInput.close();
        audioInput.close();
        videoPrimed = false;
        audioPrimed = false;
        presentationOutOfSyncWithDecoder = false;

        std::error_code ec;
        if (wantVideo && videoStream >= 0) {
            gAvcppOpenCalls.fetch_add(1);
            videoInput.openInput(path.toStdString(), ec);
            if (ec) {
                reportBurstconDiagnostic(QStringLiteral("video openInput failed path=%1").arg(path));
                return false;
            }
            videoInput.findStreamInfo(ec);
            if (ec) {
                reportBurstconDiagnostic(QStringLiteral("video findStreamInfo failed path=%1").arg(path));
                return false;
            }
            {
                const auto vst = videoInput.stream(static_cast<size_t>(videoStream));
                if (vst.isValid() && vst.timeBase().getDenominator() > 0) {
                    videoStreamTimeBase = vst.timeBase();
                } else {
                    videoStreamTimeBase = av::Rational{};
                }
            }
            if (!openVideoDecoderLocked()) {
                reportBurstconDiagnostic(QStringLiteral("video openDecoder failed path=%1").arg(path));
                return false;
            }
        }
        if (wantAudio && audioStream >= 0) {
            gAvcppOpenCalls.fetch_add(1);
            audioInput.openInput(path.toStdString(), ec);
            if (ec) {
                reportBurstconDiagnostic(QStringLiteral("audio openInput failed path=%1").arg(path));
                return false;
            }
            audioInput.findStreamInfo(ec);
            if (ec || !openAudioDecoderLocked(48000, 2, kDefaultAudioSamplesPerBurst)) {
                reportBurstconDiagnostic(QStringLiteral("audio findStreamInfo/openDecoder failed path=%1").arg(path));
                return false;
            }
        }
        loaded = (videoStream >= 0 || audioStream >= 0);
        return loaded;
    }

    BurstOriginInfo originInfo() const override
    {
        return origin;
    }

    void setPresentationVideoIndex(BurstIndex mediaBurstIndex) override
    {
        if (mediaBurstIndex < 0) {
            return;
        }
        std::lock_guard<std::mutex> lock(videoMutex);
        if (!loaded || videoStream < 0) {
            return;
        }
        const BurstIndex step = std::max<BurstIndex>(1, videoFrameStep);
        const BurstIndex aligned = (mediaBurstIndex / step) * step;
        // ConCache hits (playback + prefetch) call this without reading the demuxer. Unbounded
        // forward updates let prefetch "jump" the logical cursor past the playhead; the next real
        // decode then hits `target < videoCursorIndex` and runs seek+packet reads (visible as
        // avcpp spikes on short region loops and replays even with a full frame cache).
        if (videoCursorIndex < 0) {
            videoCursorIndex = aligned;
            return;
        }
        if (aligned < videoCursorIndex) {
            // A higher layer returned an earlier presentation time without seeking the demuxer.
            // Never move `videoCursorIndex` backward here — that desynchronizes HEVC DPB from the
            // bitstream read position. Force `decodeVideo` to seek+reopen the decoder.
            presentationOutOfSyncWithDecoder = true;
            return;
        }
        if (aligned == videoCursorIndex) {
            return;
        }
        const BurstIndex driftThreshold = 64 * step;
        if (aligned - videoCursorIndex <= driftThreshold) {
            videoCursorIndex = aligned;
        }
    }

    bool decodeVideo(BurstIndex index,
                     BurstIndex decodeOriginRequest,
                     const BurstQuality &quality,
                     bool localSkipFrames,
                     VideoBurst &out) override
    {
        using namespace av;
        std::lock_guard<std::mutex> lock(videoMutex);
        if (!loaded || videoStream < 0 || !videoDec) {
            return false;
        }
        const BurstIndex target = (index / videoFrameStep) * videoFrameStep;
        BurstIndex decodeOrigin = (target > 0) ? target : 0;
        if (decodeOriginRequest >= 0) {
            decodeOrigin = (decodeOriginRequest / videoFrameStep) * videoFrameStep;
        }
        decodeOrigin = std::clamp<BurstIndex>(decodeOrigin, 0, target);
        const BurstIndex requestedDecodeOrigin = decodeOrigin;
        const QSize rgbSize = videoRgbSizeForQuality(videoDec->width(), videoDec->height(), quality);
        const int outRgbW = std::max(1, rgbSize.width());
        const int outRgbH = std::max(1, rgbSize.height());
        const DecodedVideoFrameKey cacheLookupKey{target, outRgbW, outRgbH};
        auto cached = videoFrameCache.find(cacheLookupKey);
        if (cached != videoFrameCache.end() && !cached->second.isNull()) {
            // Do not return intra-decoder cache when the demuxer/DPB are still ahead of `target`
            // (e.g. ConCache jumped backward). A backward `videoCursorIndex` without seek corrupts HEVC.
            if (!(videoPrimed && videoCursorIndex >= 0 && target < videoCursorIndex)) {
                // Keep logical read position aligned with the frame we return; prefetch/ConCache can
                // otherwise serve frames without advancing this cursor while the demuxer runs ahead.
                videoCursorIndex = target;
                out = makeVideoBurst(target, quality, cached->second);
                return true;
            }
        }
        if (lastVideoFailedTarget == target && lastVideoFailedOrigin == decodeOrigin && videoFailureStreak >= 2) {
            return false;
        }
        const BurstIndex step = std::max<BurstIndex>(1, videoFrameStep);
        const BurstIndex driftThreshold = 64 * step;
        const bool demuxerBehindRequest =
            videoPrimed && videoCursorIndex >= 0 && (target > videoCursorIndex + driftThreshold);
        const bool needReset = !videoPrimed || target < videoCursorIndex || demuxerBehindRequest ||
            (videoPrimedOrigin != decodeOrigin && videoFrameCache.find(cacheLookupKey) == videoFrameCache.end()) ||
            presentationOutOfSyncWithDecoder;
        if (needReset) {
            // Seek to at least the requested (decodeOrigin, target) so random-access paths (strip
            // thumbnails, ConCache) land on the right GOP. A previous "first priming" rule forced
            // seek to file 0 whenever decodeOrigin>0, which left videoCursor at -frameStep, huge
            // needFrames, and demuxEof after ~1 s of 4K while the requested index was minutes in.
            const BurstIndex seekGoal = std::max<BurstIndex>(0, std::max(decodeOrigin, target));
            if (!seekVideoLocked(seekGoal)) {
                lastVideoFailedTarget = target;
                lastVideoFailedOrigin = decodeOrigin;
                ++videoFailureStreak;
                reportBurstconDiagnostic(
                    QStringLiteral("video seek failed path=%1 seekGoal=%2 target=%3 decodeOrigin=%4")
                        .arg(path)
                        .arg(seekGoal)
                        .arg(target)
                        .arg(decodeOrigin));
                return false;
            }
            videoPrimedOrigin = decodeOrigin;
        }

        const int64_t decPixels = int64_t(std::max(1, videoDec->width())) * int64_t(std::max(1, videoDec->height()));
        const int64_t kBaselinePixels = 1280LL * 720LL;
        // More coded pixels => more NALU packets per frame and often more B-frame reorder; the old flat
        // 16k cap caused "video decode exhausted" for FHD/4K (and 4KClass *RGB* output) while 720p was fine.
        const int resPacketMul = static_cast<int>(
            std::min<int64_t>(12, (decPixels + kBaselinePixels - 1) / kBaselinePixels));
        const int codecPacketMul = videoDec->raw() ? videoPacketBudgetCodecMul(videoDec->raw()->codec_id) : 1;
        const BurstIndex kPacketBudget =
            static_cast<BurstIndex>(64) * static_cast<BurstIndex>(resPacketMul) * static_cast<BurstIndex>(codecPacketMul);

        struct VideoPassStats {
            QString stopReason;
            int kMaxDemux = 0;
            int kMaxVideoPackets = 0;
            BurstIndex cursorBase = 0;
            BurstIndex framesNeeded = 0;
            int demuxReads = 0;
            int videoPackets = 0;
            int nDecodeNullIncomplete = 0;
            int nRescaleFail = 0;
            int nQimageNull = 0;
            int nSkipPolicy = 0;
            int nPushed = 0;
            qint64 minPushedIndex = -1;
            qint64 maxPushedIndex = -1;
            qint64 nearestAbs = -1;
            int cacheSize = 0;
            bool usedNearest = false;
            int outW = 0, outH = 0, decW = 0, decH = 0;
            qint64 maxSlack = 0;
        };

        auto decodeForward = [&](QImage &picked, BurstIndex &pickedIndex, VideoPassStats &ps) {
            ps = VideoPassStats();
            std::error_code ec;
            int skipCounter = 0;
            const BurstIndex cursorBase = std::max<BurstIndex>(-videoFrameStep, videoCursorIndex);
            const BurstIndex framesNeeded =
                std::max<BurstIndex>(1, ((target - cursorBase) / std::max<BurstIndex>(1, videoFrameStep)) + 2);
            // Budget must count *video* packets only: the file demuxer returns interleaved audio/subtitle/etc.
            // packets on the same FormatContext. Capping total readPacket() calls caused "decode exhausted"
            // on dense interleave (common on high-bitrate FullHD) while the decoder was still catching up.
            const int maxVideoPackets = static_cast<int>(std::clamp<BurstIndex>(
                framesNeeded * kPacketBudget, 512, static_cast<BurstIndex>(1048576)));
            const int kMaxDemuxReads = (resPacketMul > 1) ? 524288 : 131072;
            int demuxReads = 0;
            int videoPackets = 0;
            ps.kMaxDemux = kMaxDemuxReads;
            ps.kMaxVideoPackets = maxVideoPackets;
            ps.cursorBase = cursorBase;
            ps.framesNeeded = framesNeeded;
            ps.outW = outRgbW;
            ps.outH = outRgbH;
            ps.decW = videoDec->width();
            ps.decH = videoDec->height();
            AVCodecContext* const vctx = videoDec->raw();
            if (!vctx) {
                ps.stopReason = QStringLiteral("noCodecCtx");
            } else {
            struct AvfRaii {
                AVFrame* f{nullptr};
                ~AvfRaii()
                {
                    if (f) {
                        av_frame_free(&f);
                    }
                }
            } avfRaii;
            avfRaii.f = av_frame_alloc();
            if (!avfRaii.f) {
                ps.stopReason = QStringLiteral("av_frame_alloc");
            } else {
            AVFrame* const avf = avfRaii.f;
            const av::Rational tsRForBets = videoStreamTimeBase.getDenominator() > 0
                ? videoStreamTimeBase
                : videoDec->timeBase();

            bool stopDecodeForward = false;
            const auto applyOneDecoded = [&](av::VideoFrame& frame, qint64 packetTsMs) {
                qint64 frameMs = -1;
                {
                    const int64_t bet = av::frame::get_best_effort_timestamp(frame.raw());
                    if (bet != av::NoPts && tsRForBets.getDenominator() > 0) {
                        frameMs = timestampToMs(av::Timestamp(bet, tsRForBets));
                    }
                }
                if (frameMs < 0) {
                    frameMs = timestampToMs(frame.pts());
                }
                if (frameMs < 0) {
                    frameMs = (packetTsMs >= 0) ? packetTsMs : burstIndexToMs(target);
                }
                const BurstIndex frameIndex = (msToBurstIndex(frameMs) / videoFrameStep) * videoFrameStep;
                if ((skipFrames || localSkipFrames) && frameIndex < target && (++skipCounter % 2 == 1)) {
                    ++ps.nSkipPolicy;
                    return;
                }
                const int dstW = outRgbW;
                const int dstH = outRgbH;
                const int codedW = frame.width();
                const int codedH = frame.height();
                const AVPixelFormat srcPixFmt = static_cast<AVPixelFormat>(frame.raw()->format);
                const bool wantsAlpha = avPixelFormatHasAlpha(srcPixFmt);
                if (codedW <= 0 || codedH <= 0) {
                    return;
                }
                if (!videoRescaler || codedW != scalerSrcW || codedH != scalerSrcH ||
                    wantsAlpha != scalerOutputsAlpha) {
                    const AVPixelFormat dstPixFmt = wantsAlpha ? AV_PIX_FMT_BGRA : AV_PIX_FMT_RGB24;
                    videoRescaler = std::make_unique<av::VideoRescaler>(
                        codedW, codedH, dstPixFmt, av::SwsFlagFastBilinear);
                    scalerSrcW = codedW;
                    scalerSrcH = codedH;
                    scalerOutputsAlpha = wantsAlpha;
                }
                std::error_code rescaleEc;
                QImage img;
                if (wantsAlpha) {
                    av::VideoFrame bgraFrame{AV_PIX_FMT_BGRA, dstW, dstH};
                    videoRescaler->rescale(bgraFrame, frame, rescaleEc);
                    if (rescaleEc) {
                        ++ps.nRescaleFail;
                        rescaleEc.clear();
                        return;
                    }
                    img = QImage(dstW, dstH, QImage::Format_ARGB32);
                    if (img.isNull() || !img.bits()) {
                        ++ps.nQimageNull;
                        return;
                    }
                    const int dstStride = img.bytesPerLine();
                    const int srcStride = bgraFrame.raw()->linesize[0];
                    const int copyStride = std::min(dstStride, srcStride);
                    auto* srcBytes = bgraFrame.data(0);
                    auto* dstBytes = img.bits();
                    if (dstStride == srcStride && copyStride == dstW * 4) {
                        std::memcpy(dstBytes, srcBytes, static_cast<size_t>(copyStride) * static_cast<size_t>(dstH));
                    } else {
                        for (int y = 0; y < dstH; ++y) {
                            std::memcpy(dstBytes + y * dstStride, srcBytes + y * srcStride, static_cast<size_t>(copyStride));
                        }
                    }
                } else {
                    av::VideoFrame rgbFrame{AV_PIX_FMT_RGB24, dstW, dstH};
                    videoRescaler->rescale(rgbFrame, frame, rescaleEc);
                    if (rescaleEc) {
                        ++ps.nRescaleFail;
                        rescaleEc.clear();
                        return;
                    }
                    img = QImage(rgbFrame.width(), rgbFrame.height(), QImage::Format_RGB888);
                    if (img.isNull() || !img.bits()) {
                        ++ps.nQimageNull;
                        return;
                    }
                    const int dstStride = img.bytesPerLine();
                    const int srcStride = rgbFrame.raw()->linesize[0];
                    const int copyStride = std::min(dstStride, srcStride);
                    auto* srcBytes = rgbFrame.data(0);
                    auto* dstBytes = img.bits();
                    if (dstStride == srcStride && copyStride == dstW * 3) {
                        std::memcpy(dstBytes, srcBytes, static_cast<size_t>(copyStride) * static_cast<size_t>(dstH));
                    } else {
                        for (int y = 0; y < dstH; ++y) {
                            std::memcpy(dstBytes + y * dstStride, srcBytes + y * srcStride, static_cast<size_t>(copyStride));
                        }
                    }
                }
                const DecodedVideoFrameKey storeKey{frameIndex, dstW, dstH};
                videoFrameCache[storeKey] = img;
                ++ps.nPushed;
                if (ps.minPushedIndex < 0 || frameIndex < ps.minPushedIndex) {
                    ps.minPushedIndex = static_cast<qint64>(frameIndex);
                }
                if (ps.maxPushedIndex < 0 || frameIndex > ps.maxPushedIndex) {
                    ps.maxPushedIndex = static_cast<qint64>(frameIndex);
                }
                videoFrameBytesUsed += imageBytes(img);
                while (videoFrameCache.size() > kDecoderVideoFrameCacheMaxFrames ||
                       videoFrameBytesUsed > kDecoderVideoFrameCacheMaxBytes) {
                    if (videoFrameCache.empty()) {
                        break;
                    }
                    const BurstIndex ringLo = target - 2 * step;
                    const BurstIndex ringHi = target + 64 * step;
                    auto itVictim = videoFrameCache.end();
                    for (auto it = videoFrameCache.begin(); it != videoFrameCache.end(); ++it) {
                        const BurstIndex idx = it->first.index;
                        if (idx < ringLo || idx > ringHi) {
                            if (itVictim == videoFrameCache.end() || idx < itVictim->first.index) {
                                itVictim = it;
                            }
                        }
                    }
                    if (itVictim == videoFrameCache.end()) {
                        qint64 bestDist = -1;
                        for (auto it = videoFrameCache.begin(); it != videoFrameCache.end(); ++it) {
                            const qint64 di = it->first.index - target;
                            const qint64 d = di >= 0 ? di : -di;
                            if (d > bestDist) {
                                bestDist = d;
                                itVictim = it;
                            }
                        }
                    }
                    videoFrameBytesUsed -= std::min<uint64_t>(videoFrameBytesUsed, imageBytes(itVictim->second));
                    videoFrameCache.erase(itVictim);
                }
                videoCursorIndex = frameIndex;
                if (frameIndex >= target) {
                    picked = img;
                    pickedIndex = frameIndex;
                    ps.demuxReads = demuxReads;
                    ps.videoPackets = videoPackets;
                    ps.stopReason = QStringLiteral("pickedInLoop");
                    stopDecodeForward = true;
                }
            };

            const auto receiveAndProcess = [&](qint64 packetTsMs) {
                for (;;) {
                    if (stopDecodeForward) {
                        return;
                    }
                    const int r = avcodec_receive_frame(vctx, avf);
                    if (r == 0) {
                        gAvcppVideoDecodeCalls.fetch_add(1);
                        av::VideoFrame frame{avf};
                        if (tsRForBets.getDenominator() > 0) {
                            frame.setTimeBase(tsRForBets);
                        }
                        frame.setStreamIndex(videoStream);
                        frame.setComplete(true);
                        if (frame.isNull() || !frame.isValid() || !frame.isComplete()) {
                            ++ps.nDecodeNullIncomplete;
                        } else {
                            applyOneDecoded(frame, packetTsMs);
                        }
                        continue; // more decoder-delayed or multi-output frames
                    }
                    if (r == AVERROR(EAGAIN) || r == AVERROR_EOF) {
                        return;
                    }
                    ++ps.nDecodeNullIncomplete;
                    return;
                }
            };

            while (demuxReads < kMaxDemuxReads && !stopDecodeForward) {
                if (videoPackets >= maxVideoPackets) {
                    ps.stopReason = QStringLiteral("videoPacketLimit");
                    break;
                }
                Packet pkt = videoInput.readPacket(ec);
                ++demuxReads;
                gAvcppPacketReads.fetch_add(1);
                if (ec) {
                    ps.stopReason = QStringLiteral("packetReadError");
                    ec.clear();
                    break;
                }
                if (!pkt) {
                    (void) avcodec_send_packet(vctx, nullptr);
                    receiveAndProcess(burstIndexToMs(target));
                    if (stopDecodeForward) {
                        // picked set inside applyOneDecoded
                    } else {
                        ps.stopReason = QStringLiteral("demuxEof");
                    }
                    break;
                }
                if (pkt.streamIndex() != videoStream) {
                    continue;
                }
                const qint64 packetTsMs = packetTimestampToMs(pkt);
                for (int sp = 0; sp < 8; ++sp) {
                    const int s = avcodec_send_packet(vctx, pkt.raw());
                    if (s == 0) {
                        ++videoPackets;
                        receiveAndProcess(packetTsMs);
                        if (stopDecodeForward) {
                            break;
                        }
                        break; // from send-retry
                    }
                    if (s == AVERROR(EAGAIN)) {
                        receiveAndProcess(packetTsMs);
                        if (stopDecodeForward) {
                            break;
                        }
                        continue; // drain then retry
                    }
                    ++ps.nDecodeNullIncomplete;
                    break;
                }
                if (stopDecodeForward) {
                    break;
                }
            }
            }
            }
            ps.demuxReads = demuxReads;
            ps.videoPackets = videoPackets;
            if (ps.stopReason.isEmpty() && demuxReads >= kMaxDemuxReads) {
                ps.stopReason = QStringLiteral("demuxReadLimit");
            }
            if (ps.stopReason.isEmpty()) {
                ps.stopReason = QStringLiteral("readLoopEnd");
            }
            if (picked.isNull()) {
                // Exact (target, dstW, dstH) is ideal; FHD/4K + container timing can land on a nearby
                // burst grid point so lower_bound(==) often misses even when frames are in the cache.
                const qint64 maxSlack = 32 * static_cast<qint64>(std::max<BurstIndex>(1, videoFrameStep));
                ps.maxSlack = maxSlack;
                ps.cacheSize = static_cast<int>(videoFrameCache.size());
                qint64 bestAbs = std::numeric_limits<qint64>::max();
                BurstIndex bestIdx = -1;
                for (const auto &entry : videoFrameCache) {
                    if (entry.first.rgbW != outRgbW || entry.first.rgbH != outRgbH) {
                        continue;
                    }
                    if (entry.second.isNull()) {
                        continue;
                    }
                    const qint64 d = std::llabs(static_cast<qint64>(entry.first.index) -
                                                static_cast<qint64>(target));
                    if (d < bestAbs && d <= maxSlack) {
                        bestAbs = d;
                        bestIdx = entry.first.index;
                        picked = entry.second;
                    }
                }
                if (bestIdx >= 0) {
                    pickedIndex = bestIdx;
                    videoCursorIndex = bestIdx;
                    ps.usedNearest = true;
                    ps.nearestAbs = bestAbs;
                    ps.stopReason = QStringLiteral("pickedNearestInCache");
                } else {
                    ps.nearestAbs = -1;
                }
            }
        };

        QImage picked;
        BurstIndex pickedIndex = -1;
        VideoPassStats vpass;
        decodeForward(picked, pickedIndex, vpass);
        if (picked.isNull()) {
            const BurstIndex resyncGoal = std::max<BurstIndex>(0, std::max(decodeOrigin, target));
            if (seekVideoLocked(resyncGoal)) {
                decodeForward(picked, pickedIndex, vpass);
            }
        }
        if (picked.isNull()) {
            lastVideoFailedTarget = target;
            lastVideoFailedOrigin = requestedDecodeOrigin;
            ++videoFailureStreak;
            reportBurstconDiagnostic(
                QStringLiteral("video decode exhausted path=%1 target=%2 decodeOrigin=%3 step=%4 streak=%5 | "
                              "rMul=%6 outRgb=%7x%8 decCoded=%9x%10 passStop=%11 demux=%12/%13 vp=%14/%15 "
                              "needFrames~%16 curBase=%17 decNull=%18 rescaleFail=%19 qimgNull=%20 skip=%21 "
                              "pushed=%22 idx[min..max]=%23..%24 cache=%25 nearest=%26 nearestD=%27 maxSlack=%28")
                    .arg(path)
                    .arg(target)
                    .arg(requestedDecodeOrigin)
                    .arg(videoFrameStep)
                    .arg(videoFailureStreak)
                    .arg(resPacketMul)
                    .arg(vpass.outW)
                    .arg(vpass.outH)
                    .arg(vpass.decW)
                    .arg(vpass.decH)
                    .arg(vpass.stopReason)
                    .arg(vpass.demuxReads)
                    .arg(vpass.kMaxDemux)
                    .arg(vpass.videoPackets)
                    .arg(vpass.kMaxVideoPackets)
                    .arg(vpass.framesNeeded)
                    .arg(vpass.cursorBase)
                    .arg(vpass.nDecodeNullIncomplete)
                    .arg(vpass.nRescaleFail)
                    .arg(vpass.nQimageNull)
                    .arg(vpass.nSkipPolicy)
                    .arg(vpass.nPushed)
                    .arg(vpass.minPushedIndex)
                    .arg(vpass.maxPushedIndex)
                    .arg(vpass.cacheSize)
                    .arg(vpass.usedNearest ? 1 : 0)
                    .arg(vpass.nearestAbs)
                    .arg(vpass.maxSlack));
            return false;
        }
        lastVideoFailedTarget = -1;
        lastVideoFailedOrigin = -1;
        videoFailureStreak = 0;
        out = makeVideoBurst(pickedIndex, quality, picked);
        return true;
    }

    bool decodeAudio(BurstIndex index,
                     BurstIndex decodeOriginRequest,
                     int sampleRate,
                     int channels,
                     int samplesPerBurst,
                     AudioBurst &out) override
    {
        using namespace av;
        std::lock_guard<std::mutex> lock(audioMutex);
        if (!loaded || audioStream < 0 || !audioDec || !audioResampler) {
            return false;
        }
        const int outRate = std::max(1, sampleRate);
        const int outChannels = std::max(1, channels);
        const int outSamples = std::max(1, samplesPerBurst);
        if (outRate != audioOutRate || outChannels != audioOutChannels || outSamples != audioOutSamplesPerBurst) {
            if (!openAudioDecoderLocked(outRate, outChannels, outSamples)) {
                reportBurstconDiagnostic(
                    QStringLiteral("audio decoder reopen failed path=%1 rate=%2 ch=%3").arg(path).arg(outRate).arg(outChannels));
                return false;
            }
        }
        const BurstIndex audioStep = std::max<BurstIndex>(1, (kBurstTimebaseHz * audioOutSamplesPerBurst) / audioOutRate);
        const BurstIndex target = (index / audioStep) * audioStep;
        BurstIndex decodeOrigin = (target > 0) ? target : 0;
        if (decodeOriginRequest >= 0) {
            decodeOrigin = (decodeOriginRequest / audioStep) * audioStep;
        }
        decodeOrigin = std::clamp<BurstIndex>(decodeOrigin, 0, target);
        auto cached = audioBurstCache.find(target);
        if (cached != audioBurstCache.end()) {
            out = cached->second;
            return true;
        }
        if (lastAudioFailedTarget == target && lastAudioFailedOrigin == decodeOrigin && audioFailureStreak >= 2) {
            return false;
        }
        const bool needReset = !audioPrimed ||
            target < audioCursorIndex ||
            (audioPrimedOrigin != decodeOrigin && audioBurstCache.find(target) == audioBurstCache.end());
        if (needReset) {
            if (!seekAudioLocked(decodeOrigin)) {
                lastAudioFailedTarget = target;
                lastAudioFailedOrigin = decodeOrigin;
                ++audioFailureStreak;
                reportBurstconDiagnostic(QStringLiteral("audio seek failed path=%1 target=%2 decodeOrigin=%3")
                                              .arg(path)
                                              .arg(target)
                                              .arg(decodeOrigin));
                return false;
            }
        }

        const size_t needed = static_cast<size_t>(audioOutSamplesPerBurst * audioOutChannels);
        std::error_code ec;
        while (audioCursorIndex <= target) {
            if (audioBurstCache.find(audioCursorIndex) == audioBurstCache.end()) {
                while (audioPending.size() < needed) {
                    Packet pkt = audioInput.readPacket(ec);
                    gAvcppPacketReads.fetch_add(1);
                    if (ec || !pkt) {
                        ec.clear();
                        break;
                    }
                    if (pkt.streamIndex() != audioStream) {
                        continue;
                    }
                    AudioSamples decoded = audioDec->decode(pkt, ec);
                    gAvcppAudioDecodeCalls.fetch_add(1);
                    if (ec || !decoded) {
                        ec.clear();
                        continue;
                    }
                    audioResampler->push(decoded, ec);
                    if (ec) {
                        ec.clear();
                        continue;
                    }
                    while (true) {
                        AudioSamples popped(AV_SAMPLE_FMT_S16, 1024, audioOutChannelLayout, audioOutRate);
                        const bool ok = audioResampler->pop(popped, false, ec);
                        if (ec || !ok || !popped) {
                            ec.clear();
                            break;
                        }
                        const int count = popped.samplesCount() * popped.channelsCount();
                        const int16_t *pcm = reinterpret_cast<const int16_t *>(popped.data(0));
                        audioPending.insert(audioPending.end(), pcm, pcm + count);
                    }
                }
                AudioBurst burst;
                burst.index = audioCursorIndex;
                burst.sampleRate = audioOutRate;
                burst.channels = audioOutChannels;
                burst.samples.assign(needed, 0);
                for (size_t i = 0; i < needed && !audioPending.empty(); ++i) {
                    burst.samples[i] = static_cast<int32_t>(audioPending.front());
                    audioPending.pop_front();
                }
                audioBurstCache[audioCursorIndex] = burst;
                if (audioBurstCache.size() > 256) {
                    audioBurstCache.erase(audioBurstCache.begin());
                }
            }
            audioCursorIndex += audioStep;
        }
        lastAudioFailedTarget = -1;
        lastAudioFailedOrigin = -1;
        audioFailureStreak = 0;
        out = audioBurstCache[target];
        return true;
    }

private:
    bool openVideoDecoderLocked()
    {
        using namespace av;
        if (videoStream < 0) {
            return false;
        }
        std::error_code ec;
        auto st = videoInput.stream(static_cast<size_t>(videoStream));
        if (st.isNull()) {
            reportBurstconDiagnostic(QStringLiteral("video stream null path=%1").arg(path));
            return false;
        }
        auto dec = std::make_unique<VideoDecoderContext>(st);
        Codec codec = findDecodingCodec(dec->raw()->codec_id);
        dec->setCodec(codec);
        dec->setRefCountedFrames(true);
        int threadCount = static_cast<int>(std::thread::hardware_concurrency());
        if (threadCount < 1) {
            threadCount = 1;
        }
        threadCount = std::min(threadCount, 12);
        // High internal thread count can delay the first `avcodec_receive_frame` result relative to
        // packet count; 4K VP9+MP4 is sensitive, and the decode loop now drains with send/receive.
        if (dec->raw() && (dec->raw()->codec_id == AV_CODEC_ID_VP9 || dec->raw()->codec_id == AV_CODEC_ID_AV1)) {
            threadCount = 1;
        }
        const std::string threadsOpt = std::to_string(threadCount);
        dec->open(Dictionary{{"threads", threadsOpt.c_str()}}, ec);
        if (ec) {
            reportBurstconDiagnostic(
                QStringLiteral("video decoder open failed path=%1 (%2x%3)")
                    .arg(path)
                    .arg(dec->width())
                    .arg(dec->height()));
            return false;
        }
        videoDec = std::move(dec);
        videoRescaler.reset();
        scalerSrcW = 0;
        scalerSrcH = 0;
        scalerOutputsAlpha = false;
        return true;
    }

    bool openAudioDecoderLocked(int outRate, int outChannels, int outSamplesPerBurst)
    {
        using namespace av;
        if (audioStream < 0) {
            return false;
        }
        std::error_code ec;
        auto st = audioInput.stream(static_cast<size_t>(audioStream));
        if (st.isNull()) {
            return false;
        }
        auto dec = std::make_unique<AudioDecoderContext>(st);
        Codec codec = findDecodingCodec(dec->raw()->codec_id);
        dec->setCodec(codec);
        dec->open(ec);
        if (ec) {
            return false;
        }
        audioOutRate = outRate;
        audioOutChannels = outChannels;
        audioOutSamplesPerBurst = outSamplesPerBurst;
        audioOutChannelLayout = (audioOutChannels == 1) ? AV_CH_LAYOUT_MONO : AV_CH_LAYOUT_STEREO;
        auto resampler = std::make_unique<AudioResampler>(audioOutChannelLayout,
                                                          audioOutRate,
                                                          AV_SAMPLE_FMT_S16,
                                                          dec->channelLayout(),
                                                          dec->sampleRate(),
                                                          dec->sampleFormat(),
                                                          ec);
        if (ec) {
            return false;
        }
        audioDec = std::move(dec);
        audioResampler = std::move(resampler);
        audioPending.clear();
        return true;
    }

    /// After a demuxer seek: cheap reset vs full `openVideoDecoderLocked` (reopen is costly at 4K).
    void flushVideoDecoderAfterSeekLocked()
    {
        AVCodecContext *const vctx = videoDec ? videoDec->raw() : nullptr;
        if (!vctx) {
            return;
        }
        avcodec_flush_buffers(vctx);
        AVFrame *drain = av_frame_alloc();
        if (!drain) {
            return;
        }
        while (true) {
            const int r = avcodec_receive_frame(vctx, drain);
            if (r == AVERROR(EAGAIN) || r == AVERROR_EOF) {
                break;
            }
        }
        av_frame_free(&drain);
    }

    bool seekVideoLocked(BurstIndex origin)
    {
        using namespace av;
        std::error_code ec;
        const BurstIndex originB = std::max<BurstIndex>(0, origin);
        const BurstIndex step = std::max<BurstIndex>(1, videoFrameStep);
        const BurstIndex targetAligned = (step > 0) ? ((originB / step) * step) : originB;
        videoFrameCache.clear();
        videoFrameBytesUsed = 0;
        bool demuxerSeekOk = !videoInput.seekable();
        int seekRet = -1;
        if (videoInput.seekable()) {
            demuxerSeekOk = false;
            AVFormatContext *const fc = videoInput.raw();
            if (fc && videoStream >= 0 && videoStream < static_cast<int>(fc->nb_streams)) {
                AVStream *const st = fc->streams[videoStream];
                const qint64 seekMs = std::max<qint64>(0, burstIndexToMs(targetAligned));
                const AVRational msTb{1, 1000};
                const int64_t wantedTb = av_rescale_q(seekMs, msTb, st->time_base);

                // Prefer container sync index (moov etc.): tight byte seek. No custom file scan.
                // (AVStream::index_entries is not public in newer FFmpeg; use avformat_index_*.)
#if LIBAVFORMAT_VERSION_INT >= AV_VERSION_INT(58, 78, 100)
                if (avformat_index_get_entries_count(st) > 0) {
                    const int ix = av_index_search_timestamp(st, wantedTb, AVINDEX_KEYFRAME);
                    if (ix >= 0) {
                        const AVIndexEntry *const idxEnt = avformat_index_get_entry(st, ix);
                        if (idxEnt) {
                            const int64_t bytePos = idxEnt->pos;
                            if (bytePos >= 0) {
                                seekRet = av_seek_frame(fc, videoStream, bytePos, AVSEEK_FLAG_BYTE);
                            }
                        }
                    }
                }
#else
                if (st->index_entries && st->nb_index_entries > 0) {
                    const int ix = av_index_search_timestamp(st, wantedTb, AVINDEX_KEYFRAME);
                    if (ix >= 0) {
                        const int64_t bytePos = st->index_entries[ix].pos;
                        if (bytePos >= 0) {
                            seekRet = av_seek_frame(fc, videoStream, bytePos, AVSEEK_FLAG_BYTE);
                        }
                    }
                }
#endif
                if (seekRet < 0) {
                    seekRet = avformat_seek_file(fc, videoStream, INT64_MIN, wantedTb, wantedTb, AVSEEK_FLAG_BACKWARD);
                }
                if (seekRet < 0) {
                    auto vst = videoInput.stream(static_cast<size_t>(videoStream));
                    if (vst.isValid() && vst.timeBase().getDenominator() > 0) {
                        const Timestamp ts{std::chrono::milliseconds(seekMs)};
                        const int64_t pos = ts.timestamp(vst.timeBase());
                        videoInput.seek(pos, static_cast<int>(videoStream), AVSEEK_FLAG_BACKWARD, ec);
                        demuxerSeekOk = !ec;
                        if (ec) {
                            ec.clear();
                        }
                    }
                } else {
                    demuxerSeekOk = true;
                }
            }
        }
        // Full decoder reopen is correct but very slow for 4K. Flush+drain is enough after seek for most
        // codecs; keep reopen for multi-threaded H.264 where flush is still unreliable (see FFmpeg notes).
        const AVCodecContext *const vctxProbe = videoDec ? videoDec->raw() : nullptr;
        const bool h264Multithreaded =
            vctxProbe && vctxProbe->codec_id == AV_CODEC_ID_H264 && vctxProbe->thread_count > 1;
        if (!videoDec || !demuxerSeekOk || h264Multithreaded) {
            if (!openVideoDecoderLocked()) {
                reportBurstconDiagnostic(
                    QStringLiteral("video decoder reopen after seek failed path=%1").arg(path));
                return false;
            }
        } else {
            flushVideoDecoderAfterSeekLocked();
        }
        videoCursorIndex = targetAligned - videoFrameStep;
        videoPrimed = true;
        videoPrimedOrigin = origin;
        presentationOutOfSyncWithDecoder = false;
        lastVideoFailedTarget = -1;
        lastVideoFailedOrigin = -1;
        videoFailureStreak = 0;
        return true;
    }

    bool seekAudioLocked(BurstIndex origin)
    {
        using namespace av;
        std::error_code ec;
        BurstIndex seekIndex = std::max<BurstIndex>(0, origin);
        if (audioInput.seekable()) {
            bool didByteSeek = false;
            if (sharedFile) {
                const PacketIndexEntry start = sharedFile->audioStartFor(seekIndex);
                seekIndex = start.index;
                if (start.bytePos >= 0) {
                    audioInput.seek(start.bytePos, -1, AVSEEK_FLAG_BYTE, ec);
                    didByteSeek = !ec;
                }
            }
            if (ec) {
                ec.clear();
            }
            if (!didByteSeek) {
                const qint64 seekMs = std::max<qint64>(0, burstIndexToMs(seekIndex));
                auto ast = audioInput.stream(static_cast<size_t>(audioStream));
                if (ast.isValid() && ast.timeBase().getDenominator() > 0) {
                    const Timestamp ts{std::chrono::milliseconds(seekMs)};
                    const int64_t pos = ts.timestamp(ast.timeBase());
                    audioInput.seek(pos, static_cast<int>(audioStream), AVSEEK_FLAG_BACKWARD, ec);
                }
                if (ec) {
                    ec.clear();
                }
            }
        }
        if (!openAudioDecoderLocked(audioOutRate, audioOutChannels, audioOutSamplesPerBurst)) {
            return false;
        }
        const BurstIndex step = std::max<BurstIndex>(1, (kBurstTimebaseHz * audioOutSamplesPerBurst) / audioOutRate);
        audioCursorIndex = (seekIndex / step) * step;
        audioPrimed = true;
        audioPrimedOrigin = (origin / step) * step;
        lastAudioFailedTarget = -1;
        lastAudioFailedOrigin = -1;
        audioFailureStreak = 0;
        return true;
    }

    VideoBurst makeVideoBurst(BurstIndex index, const BurstQuality &quality, const QImage &src) const
    {
        QImage img = src;
        if (quality.width > 0 && quality.height > 0 &&
            (img.width() != quality.width || img.height() != quality.height)) {
            img = img.scaled(quality.width, quality.height, Qt::KeepAspectRatio, Qt::FastTransformation);
        }
        return VideoBurst{index, BurstQuality{img.width(), img.height()}, img};
    }

private:
    std::shared_ptr<SharedMediaFile> sharedFile;
    QString path;
    bool loaded = false;
    bool skipFrames = false;
    bool wantVideo = true;
    bool wantAudio = true;
    mutable std::mutex videoMutex;
    mutable std::mutex audioMutex;
    av::FormatContext videoInput;
    av::FormatContext audioInput;
    int videoStream = -1;
    int audioStream = -1;
    av::Rational videoStreamTimeBase;
    std::unique_ptr<av::VideoDecoderContext> videoDec;
    std::unique_ptr<av::AudioDecoderContext> audioDec;
    std::unique_ptr<av::VideoRescaler> videoRescaler;
    std::unique_ptr<av::AudioResampler> audioResampler;
    uint64_t audioOutChannelLayout = AV_CH_LAYOUT_STEREO;
    int audioOutRate = 48000;
    int audioOutChannels = 2;
    int audioOutSamplesPerBurst = kDefaultAudioSamplesPerBurst;
    BurstIndex videoFrameStep = std::max<BurstIndex>(1, kBurstTimebaseHz / 30);
    BurstIndex videoCursorIndex = -1;
    BurstIndex audioCursorIndex = -1;
    bool videoPrimed = false;
    bool audioPrimed = false;
    BurstIndex videoPrimedOrigin = -1;
    BurstIndex audioPrimedOrigin = -1;
    BurstIndex lastVideoFailedTarget = -1;
    BurstIndex lastVideoFailedOrigin = -1;
    int videoFailureStreak = 0;
    BurstIndex lastAudioFailedTarget = -1;
    BurstIndex lastAudioFailedOrigin = -1;
    int audioFailureStreak = 0;
    std::map<DecodedVideoFrameKey, QImage> videoFrameCache;
    uint64_t videoFrameBytesUsed = 0;
    int scalerSrcW = 0;
    int scalerSrcH = 0;
    bool scalerOutputsAlpha = false;
    std::map<BurstIndex, AudioBurst> audioBurstCache;
    std::deque<int16_t> audioPending;
    BurstOriginInfo origin;
    /// Set when presentation time moves backward vs demuxer (e.g. ConCache hit); cleared after seek.
    bool presentationOutOfSyncWithDecoder = false;
};

} // namespace

std::shared_ptr<IMediaDecoderBackend> makeAvcppMediaDecoderBackend(std::shared_ptr<SharedMediaFile> sharedFile,
                                                                  bool skipFrames,
                                                                  bool wantVideo,
                                                                  bool wantAudio)
{
    return std::make_shared<AvcppMediaDecoderBackend>(std::move(sharedFile), skipFrames, wantVideo, wantAudio);
}

std::shared_ptr<SharedMediaFile> ConMediaPool::acquireSharedMediaFile(const QString &path)
{
    const std::string key = path.toStdString();
    auto it = sharedFilesByPath.find(key);
    if (it != sharedFilesByPath.end()) {
        return it->second;
    }
    auto sharedFile = std::make_shared<SharedMediaFile>();
    sharedFile->path = path;
    sharedFilesByPath[key] = sharedFile;
    return sharedFile;
}

} // namespace burstcon
