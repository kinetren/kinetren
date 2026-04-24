#include "burstcon.h"

#include <QPainter>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <deque>
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
#include <avcpp/videorescaler.h>

namespace burstcon {
namespace {

constexpr int kDefaultAudioSamplesPerBurst = 1024;
const av::Rational kMillisecondsTimebase{1, 1000};

std::atomic<uint64_t> gAvcppOpenCalls{0};
std::atomic<uint64_t> gAvcppPacketReads{0};
std::atomic<uint64_t> gAvcppVideoDecodeCalls{0};
std::atomic<uint64_t> gAvcppAudioDecodeCalls{0};
std::atomic<uint64_t> gCacheHits{0};
std::atomic<uint64_t> gCacheMisses{0};
std::atomic<int64_t> gCacheEntries{0};

int64_t qualityArea(const BurstQuality &q)
{
    return static_cast<int64_t>(std::max(1, q.width)) * static_cast<int64_t>(std::max(1, q.height));
}

qint64 timestampToMs(const av::Timestamp &ts)
{
    if (ts.isNoPts()) {
        return -1;
    }
    return ts.timestamp(kMillisecondsTimebase);
}

uint64_t imageBytes(const QImage &img)
{
#if QT_VERSION >= QT_VERSION_CHECK(5, 10, 0)
    return static_cast<uint64_t>(img.sizeInBytes());
#else
    return static_cast<uint64_t>(img.byteCount());
#endif
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
    if (tsMs > 0) {
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

struct SharedMediaFile {
    QString path;
    std::mutex mutex;
    bool indexed = false;
    BurstOriginInfo origin;
    int videoStream = -1;
    int audioStream = -1;
    BurstIndex videoFrameStep = std::max<BurstIndex>(1, kBurstTimebaseHz / 30);
    std::vector<PacketIndexEntry> videoKeyframes;
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
        qint64 lastAudioIndexedMs = std::numeric_limits<qint64>::min();
        constexpr qint64 kAudioIndexMinIntervalMs = 400;
        while (true) {
            Packet pkt = input.readPacket(ec);
            gAvcppPacketReads.fetch_add(1);
            if (ec || !pkt) {
                ec.clear();
                break;
            }
            if (pkt.streamIndex() == videoStream && pkt.isKeyPacket()) {
                PacketIndexEntry entry = makePacketIndexEntry(pkt);
                if (entry.bytePos >= 0) {
                    videoKeyframes.push_back(entry);
                }
            } else if (pkt.streamIndex() == audioStream) {
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
        if (videoKeyframes.empty()) {
            videoKeyframes.push_back(PacketIndexEntry{0, 0});
        }
        if (audioPackets.empty()) {
            audioPackets.push_back(PacketIndexEntry{0, 0});
        }
        dedupePacketIndex(videoKeyframes);
        dedupePacketIndex(audioPackets);
        indexed = true;
    }

    PacketIndexEntry videoStartFor(BurstIndex target) const
    {
        if (videoKeyframes.empty()) {
            return PacketIndexEntry{0, 0};
        }
        auto it = std::upper_bound(videoKeyframes.begin(), videoKeyframes.end(), target,
                                   [](BurstIndex value, const PacketIndexEntry &entry) {
                                       return value < entry.index;
                                   });
        if (it == videoKeyframes.begin()) {
            return *it;
        }
        --it;
        return *it;
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
        videoInput.close();
        audioInput.close();
        videoPrimed = false;
        audioPrimed = false;

        std::error_code ec;
        if (wantVideo && videoStream >= 0) {
            gAvcppOpenCalls.fetch_add(1);
            videoInput.openInput(path.toStdString(), ec);
            if (ec) {
                return false;
            }
            videoInput.findStreamInfo(ec);
            if (ec || !openVideoDecoderLocked()) {
                return false;
            }
        }
        if (wantAudio && audioStream >= 0) {
            gAvcppOpenCalls.fetch_add(1);
            audioInput.openInput(path.toStdString(), ec);
            if (ec) {
                return false;
            }
            audioInput.findStreamInfo(ec);
            if (ec || !openAudioDecoderLocked(48000, 2, kDefaultAudioSamplesPerBurst)) {
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
        videoCursorIndex = (mediaBurstIndex / step) * step;
    }

    bool decodeVideo(BurstIndex index,
                     BurstIndex decodeOriginRequest,
                     const BurstQuality &quality,
                     bool localSkipFrames,
                     VideoBurst &out) override
    {
        using namespace av;
        std::lock_guard<std::mutex> lock(videoMutex);
        if (!loaded || videoStream < 0 || !videoDec || !videoRescaler) {
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
            // Keep logical read position aligned with the frame we return; prefetch/ConCache can
            // otherwise serve frames without advancing this cursor while the demuxer runs ahead.
            videoCursorIndex = target;
            out = makeVideoBurst(target, quality, cached->second);
            return true;
        }
        if (lastVideoFailedTarget == target && lastVideoFailedOrigin == decodeOrigin && videoFailureStreak >= 2) {
            return false;
        }
        const BurstIndex step = std::max<BurstIndex>(1, videoFrameStep);
        const BurstIndex driftThreshold = 64 * step;
        const bool demuxerBehindRequest =
            videoPrimed && videoCursorIndex >= 0 && (target > videoCursorIndex + driftThreshold);
        const bool needReset = !videoPrimed || target < videoCursorIndex || demuxerBehindRequest ||
            (videoPrimedOrigin != decodeOrigin && videoFrameCache.find(cacheLookupKey) == videoFrameCache.end());
        if (needReset) {
            // First priming of a trimmed clip: seek to 0 so GOP read is reliable. After that, seek
            // to the requested target (not only decodeOrigin) so we recover when prefetch advanced
            // the shared demuxer past the playhead or ConCache hits hid the drift.
            BurstIndex seekGoal = std::max<BurstIndex>(0, std::max(decodeOrigin, target));
            if (!videoPrimed && decodeOrigin > 0) {
                seekGoal = 0;
            }
            if (!seekVideoLocked(seekGoal)) {
                lastVideoFailedTarget = target;
                lastVideoFailedOrigin = decodeOrigin;
                ++videoFailureStreak;
                return false;
            }
            videoPrimedOrigin = decodeOrigin;
        }

        auto decodeForward = [&](QImage &picked, BurstIndex &pickedIndex) {
            std::error_code ec;
            int skipCounter = 0;
            const BurstIndex cursorBase = std::max<BurstIndex>(-videoFrameStep, videoCursorIndex);
            const BurstIndex framesNeeded =
                std::max<BurstIndex>(1, ((target - cursorBase) / std::max<BurstIndex>(1, videoFrameStep)) + 2);
            const int maxPackets = static_cast<int>(std::clamp<BurstIndex>(framesNeeded * 16, 320, 2048));
            for (int tries = 0; tries < maxPackets; ++tries) {
                Packet pkt = videoInput.readPacket(ec);
                gAvcppPacketReads.fetch_add(1);
                if (ec || !pkt) {
                    ec.clear();
                    break;
                }
                if (pkt.streamIndex() != videoStream) {
                    continue;
                }
                VideoFrame frame = videoDec->decode(pkt, ec);
                gAvcppVideoDecodeCalls.fetch_add(1);
                if (ec || frame.isNull() || !frame.isComplete()) {
                    ec.clear();
                    continue;
                }
                qint64 frameMs = timestampToMs(frame.pts());
                if (frameMs < 0) {
                    frameMs = burstIndexToMs(target);
                }
                BurstIndex frameIndex = (msToBurstIndex(frameMs) / videoFrameStep) * videoFrameStep;
                if ((skipFrames || localSkipFrames) && frameIndex < target && (++skipCounter % 2 == 1)) {
                    continue;
                }
                const QSize scaledRgb = videoRgbSizeForQuality(frame.width(), frame.height(), quality);
                const int dstW = std::max(1, scaledRgb.width());
                const int dstH = std::max(1, scaledRgb.height());
                VideoFrame rgbFrame{AV_PIX_FMT_RGB24, dstW, dstH};
                std::error_code rescaleEc;
                videoRescaler->rescale(rgbFrame, frame, rescaleEc);
                if (rescaleEc) {
                    rescaleEc.clear();
                    continue;
                }
                QImage img(rgbFrame.width(), rgbFrame.height(), QImage::Format_RGB888);
                const int dstStride = img.bytesPerLine();
                const int srcStride = rgbFrame.raw()->linesize[0];
                const int copyStride = std::min(dstStride, srcStride);
                auto *srcBytes = rgbFrame.data(0);
                auto *dstBytes = img.bits();
                if (dstStride == srcStride && copyStride == dstW * 3) {
                    std::memcpy(dstBytes, srcBytes, static_cast<size_t>(copyStride) * static_cast<size_t>(dstH));
                } else {
                    for (int y = 0; y < dstH; ++y) {
                        std::memcpy(dstBytes + y * dstStride, srcBytes + y * srcStride, static_cast<size_t>(copyStride));
                    }
                }
                const DecodedVideoFrameKey storeKey{frameIndex, dstW, dstH};
                videoFrameCache[storeKey] = img;
                videoFrameBytesUsed += imageBytes(img);
                while (videoFrameCache.size() > 192 || videoFrameBytesUsed > (256ULL * 1024ULL * 1024ULL)) {
                    if (videoFrameCache.empty()) {
                        break;
                    }
                    // Evict outside a "hot ring" around the current output target first so long
                    // playback does not delete nearby frames (std::map::begin() was min index, not LRU).
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
                    return;
                }
            }
            const DecodedVideoFrameKey nearKey{target, outRgbW, outRgbH};
            auto near = videoFrameCache.lower_bound(nearKey);
            if (picked.isNull() && near != videoFrameCache.end() && near->first.index == target &&
                near->first.rgbW == outRgbW && near->first.rgbH == outRgbH) {
                picked = near->second;
                pickedIndex = near->first.index;
                videoCursorIndex = pickedIndex;
            }
        };

        QImage picked;
        BurstIndex pickedIndex = -1;
        decodeForward(picked, pickedIndex);
        if (picked.isNull()) {
            const BurstIndex resyncGoal = std::max<BurstIndex>(0, std::max(decodeOrigin, target));
            if (seekVideoLocked(resyncGoal)) {
                videoPrimedOrigin = decodeOrigin;
                decodeForward(picked, pickedIndex);
            }
        }
        if (picked.isNull() && requestedDecodeOrigin > 0 && seekVideoLocked(0)) {
            videoPrimedOrigin = requestedDecodeOrigin;
            decodeForward(picked, pickedIndex);
        }
        if (picked.isNull()) {
            lastVideoFailedTarget = target;
            lastVideoFailedOrigin = requestedDecodeOrigin;
            ++videoFailureStreak;
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
        const std::string threadsOpt = std::to_string(threadCount);
        dec->open(Dictionary{{"threads", threadsOpt.c_str()}}, ec);
        if (ec) {
            return false;
        }
        auto scaler = std::make_unique<VideoRescaler>(
            dec->width(), dec->height(), AV_PIX_FMT_RGB24, av::SwsFlagFastBilinear);
        videoDec = std::move(dec);
        videoRescaler = std::move(scaler);
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

    bool seekVideoLocked(BurstIndex origin)
    {
        using namespace av;
        std::error_code ec;
        BurstIndex seekIndex = std::max<BurstIndex>(0, origin);
        if (videoInput.seekable()) {
            bool didByteSeek = false;
            if (sharedFile) {
                const PacketIndexEntry start = sharedFile->videoStartFor(seekIndex);
                seekIndex = start.index;
                if (start.bytePos >= 0) {
                    videoInput.seek(start.bytePos, -1, AVSEEK_FLAG_BYTE, ec);
                    didByteSeek = !ec;
                }
            }
            if (ec) {
                ec.clear();
            }
            if (!didByteSeek) {
                const qint64 seekMs = std::max<qint64>(0, burstIndexToMs(seekIndex));
                videoInput.seek(Timestamp(std::chrono::milliseconds(seekMs)), static_cast<size_t>(videoStream), true, ec);
                if (ec) {
                    ec.clear();
                }
            }
        }
        // Recreate the decoder after every seek so DPB / threading state matches a fresh
        // open at the keyframe (flush alone is not reliable with frame-threaded H.264).
        if (!openVideoDecoderLocked()) {
            return false;
        }
        videoCursorIndex = seekIndex - videoFrameStep;
        videoPrimed = true;
        videoPrimedOrigin = origin;
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
                audioInput.seek(Timestamp(std::chrono::milliseconds(seekMs)), static_cast<size_t>(audioStream), true, ec);
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
    std::map<BurstIndex, AudioBurst> audioBurstCache;
    std::deque<int16_t> audioPending;
    BurstOriginInfo origin;
};

} // namespace

bool IBurstcon::audioBurst(const BurstRequest &request, AudioBurst &out)
{
    BurstPayload payload;
    if (!burst(BurstType::Audio, request, payload) || !std::holds_alternative<AudioBurst>(payload)) {
        return false;
    }
    out = std::get<AudioBurst>(std::move(payload));
    return true;
}

bool IBurstcon::videoBurst(const BurstRequest &request, VideoBurst &out)
{
    BurstPayload payload;
    if (!burst(BurstType::Video, request, payload) || !std::holds_alternative<VideoBurst>(payload)) {
        return false;
    }
    out = std::get<VideoBurst>(std::move(payload));
    return true;
}

size_t ConCache::AudioKeyHash::operator()(const AudioKey &key) const
{
    return std::hash<int64_t>{}(key.index);
}

size_t ConCache::VideoKeyHash::operator()(const VideoKey &key) const
{
    size_t h = std::hash<int64_t>{}(key.index);
    h ^= static_cast<size_t>(key.width + 0x9e3779b9U + (h << 6U) + (h >> 2U));
    h ^= static_cast<size_t>(key.height + 0x9e3779b9U + (h << 6U) + (h >> 2U));
    return h;
}

void ConCache::notifyVideoFrameCache(bool inserted, const VideoKey &key)
{
    if (!config.onVideoFrameCacheChanged || mediaPath.isEmpty()) {
        return;
    }
    config.onVideoFrameCacheChanged(inserted, mediaPath, key.index, BurstQuality{key.width, key.height});
}

ConCache::ConCache(std::shared_ptr<IBurstcon> upstream, QString mediaPath, const ConCacheConfig &config)
    : upstream(std::move(upstream))
    , mediaPath(std::move(mediaPath))
    , config(config)
{
    if (this->config.preloadEnabled) {
        preloadThread = std::thread([this]() { preloadLoop(); });
    }
}

ConCache::~ConCache()
{
    stopPreload.store(true);
    preloadCv.notify_all();
    if (preloadThread.joinable()) {
        preloadThread.join();
    }
}

bool ConCache::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    if (type == BurstType::Audio) {
        AudioBurst audioOut;
        const AudioKey key = keyForAudio(request);
        if (getAudio(key, audioOut)) {
            out = audioOut;
            return true;
        }
        if (!upstream->audioBurst(request, audioOut)) {
            return false;
        }
        put(key, audioOut, request.index);
        out = audioOut;
        return true;
    }

    BurstRequest effective = request;
    effective.quality = effectiveVideoQuality(request);
    VideoBurst videoOut;
    const VideoKey key = keyForVideo(effective);
    if (getVideo(key, videoOut)) {
        out = videoOut;
        return true;
    }
    if (!upstream->videoBurst(effective, videoOut)) {
        return false;
    }
    put(key, videoOut, effective.index);
    out = videoOut;
    return true;
}

BurstOriginInfo ConCache::originInfo() const
{
    return upstream->originInfo();
}

void ConCache::syncPresentationVideoIndex(BurstIndex mediaBurstIndex)
{
    if (upstream) {
        upstream->syncPresentationVideoIndex(mediaBurstIndex);
    }
}

void ConCache::invalidate(const BurstRange &range)
{
    std::vector<VideoKey> videoRemoved;
    {
        upstream->invalidate(range);
        std::lock_guard<std::mutex> lock(mutex);
        for (auto it = audioEntries.begin(); it != audioEntries.end();) {
            if (it->second.index >= range.start && it->second.index <= range.end) {
                audioLru.erase(it->second.lruIt);
                gCacheEntries.fetch_sub(1);
                it = audioEntries.erase(it);
            } else {
                ++it;
            }
        }
        for (auto it = videoEntries.begin(); it != videoEntries.end();) {
            if (it->second.index >= range.start && it->second.index <= range.end) {
                videoRemoved.push_back(it->first);
                videoLru.erase(it->second.lruIt);
                videoBytesUsed -= std::min<uint64_t>(videoBytesUsed, imageBytes(it->second.value.frame));
                gCacheEntries.fetch_sub(1);
                it = videoEntries.erase(it);
            } else {
                ++it;
            }
        }
    }
    for (const auto &vk : videoRemoved) {
        notifyVideoFrameCache(false, vk);
    }
}

bool ConCache::getAudio(const AudioKey &key, AudioBurst &out)
{
    std::lock_guard<std::mutex> lock(mutex);
    auto it = audioEntries.find(key);
    if (it == audioEntries.end()) {
        gCacheMisses.fetch_add(1);
        return false;
    }
    gCacheHits.fetch_add(1);
    touchAudioLocked(key, it->second);
    out = it->second.value;
    return true;
}

bool ConCache::getVideo(const VideoKey &key, VideoBurst &out)
{
    std::unique_lock<std::mutex> lock(mutex);
    auto it = videoEntries.find(key);
    if (it == videoEntries.end()) {
        gCacheMisses.fetch_add(1);
        return false;
    }
    gCacheHits.fetch_add(1);
    touchVideoLocked(key, it->second);
    out = it->second.value;
    lock.unlock();
    if (upstream) {
        upstream->syncPresentationVideoIndex(key.index);
    }
    return true;
}

void ConCache::put(const AudioKey &key, const AudioBurst &value, BurstIndex index)
{
    std::lock_guard<std::mutex> lock(mutex);
    auto existing = audioEntries.find(key);
    if (existing != audioEntries.end()) {
        audioLru.erase(existing->second.lruIt);
        audioEntries.erase(existing);
        gCacheEntries.fetch_sub(1);
    }
    audioLru.push_front(key);
    audioEntries[key] = AudioEntry{value, audioLru.begin(), index};
    gCacheEntries.fetch_add(1);
    evictAudioLocked();
}

void ConCache::put(const VideoKey &key, const VideoBurst &value, BurstIndex index)
{
    std::vector<VideoKey> removedVideoKeys;
    {
        std::lock_guard<std::mutex> lock(mutex);
        auto existing = videoEntries.find(key);
        if (existing != videoEntries.end()) {
            removedVideoKeys.push_back(existing->first);
            videoBytesUsed -= std::min<uint64_t>(videoBytesUsed, imageBytes(existing->second.value.frame));
            videoLru.erase(existing->second.lruIt);
            videoEntries.erase(existing);
            gCacheEntries.fetch_sub(1);
        }
        videoLru.push_front(key);
        videoEntries[key] = VideoEntry{value, videoLru.begin(), index};
        videoBytesUsed += imageBytes(value.frame);
        gCacheEntries.fetch_add(1);
        evictVideoLocked(removedVideoKeys);
    }
    for (const auto &vk : removedVideoKeys) {
        notifyVideoFrameCache(false, vk);
    }
    notifyVideoFrameCache(true, key);
}

void ConCache::evictAudioLocked()
{
    while (((audioEntries.size() + videoEntries.size()) > config.maxEntries || audioEntries.size() > config.maxAudioEntries) &&
           !audioLru.empty()) {
        const AudioKey key = audioLru.back();
        audioLru.pop_back();
        audioEntries.erase(key);
        gCacheEntries.fetch_sub(1);
    }
}

void ConCache::evictVideoLocked(std::vector<VideoKey> &removedVideoKeys)
{
    while (((audioEntries.size() + videoEntries.size()) > config.maxEntries ||
            videoEntries.size() > config.maxVideoEntries ||
            videoBytesUsed > config.maxVideoBytes) &&
           !videoLru.empty()) {
        const VideoKey key = videoLru.back();
        auto it = videoEntries.find(key);
        if (it != videoEntries.end()) {
            removedVideoKeys.push_back(key);
            videoBytesUsed -= std::min<uint64_t>(videoBytesUsed, imageBytes(it->second.value.frame));
            videoEntries.erase(it);
            gCacheEntries.fetch_sub(1);
        }
        videoLru.pop_back();
    }
}

void ConCache::touchAudioLocked(const AudioKey &key, AudioEntry &entry)
{
    audioLru.erase(entry.lruIt);
    audioLru.push_front(key);
    entry.lruIt = audioLru.begin();
}

void ConCache::touchVideoLocked(const VideoKey &key, VideoEntry &entry)
{
    videoLru.erase(entry.lruIt);
    videoLru.push_front(key);
    entry.lruIt = videoLru.begin();
}

ConCache::AudioKey ConCache::keyForAudio(const BurstRequest &request) const
{
    AudioKey key;
    key.index = request.index;
    return key;
}

ConCache::VideoKey ConCache::keyForVideo(const BurstRequest &request) const
{
    VideoKey key;
    key.index = request.index;
    key.width = request.quality.width;
    key.height = request.quality.height;
    return key;
}

BurstQuality ConCache::effectiveVideoQuality(const BurstRequest &request) const
{
    BurstQuality effective = request.quality;
    const BurstOriginInfo info = upstream->originInfo();
    if (effective.width <= 0 || effective.height <= 0) {
        effective = info.sourceVideoQuality;
    }
    if (config.forcedVideoQualityEnabled && qualityArea(config.forcedVideoQuality) > 0) {
        const BurstQuality forced = config.forcedVideoQuality;
        if (qualityArea(forced) < qualityArea(info.sourceVideoQuality)) {
            effective = forced;
        } else if (qualityArea(effective) < qualityArea(info.sourceVideoQuality)) {
            effective = info.sourceVideoQuality;
        }
    }
    return effective;
}

void ConCache::schedulePreload(const BurstRequest &request, bool video, bool internalRequest)
{
    Q_UNUSED(request);
    Q_UNUSED(video);
    Q_UNUSED(internalRequest);
}

void ConCache::preloadLoop()
{
}

void ConCache::preloadBurst(BurstRequest request, bool video)
{
    Q_UNUSED(request);
    Q_UNUSED(video);
}

ConMedia::ConMedia(const QString &path, std::shared_ptr<IMediaDecoderBackend> backend)
    : path(path)
    , backend(std::move(backend))
{
    if (this->backend) {
        this->backend->load(path);
    }
}

bool ConMedia::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    std::lock_guard<std::mutex> lock(backendMutex);
    if (!backend) {
        return false;
    }
    if (type == BurstType::Audio) {
        AudioBurst audioOut;
        if (!backend->decodeAudio(request.index,
                                  request.decodeOrigin,
                                  48000,
                                  2,
                                  kDefaultAudioSamplesPerBurst,
                                  audioOut)) {
            return false;
        }
        out = audioOut;
        return true;
    }
    VideoBurst videoOut;
    if (!backend->decodeVideo(request.index, request.decodeOrigin, request.quality, false, videoOut)) {
        return false;
    }
    out = videoOut;
    return true;
}

BurstOriginInfo ConMedia::originInfo() const
{
    std::lock_guard<std::mutex> lock(backendMutex);
    if (!backend) {
        return BurstOriginInfo{};
    }
    return backend->originInfo();
}

void ConMedia::syncPresentationVideoIndex(BurstIndex mediaBurstIndex)
{
    std::lock_guard<std::mutex> lock(backendMutex);
    if (backend) {
        backend->setPresentationVideoIndex(mediaBurstIndex);
    }
}

void ConMedia::invalidate(const BurstRange &range)
{
    Q_UNUSED(range);
}

ConMediaPool::ConMediaPool(const ConMediaPoolConfig &config)
    : config(config)
{
}

std::shared_ptr<IBurstcon> ConMediaPool::acquire(const QString &path,
                                                 bool forPreview,
                                                 bool skipFrames,
                                                 BurstIndex decodeOrigin,
                                                 bool wantVideo,
                                                 bool wantAudio)
{
    Q_UNUSED(decodeOrigin);
    const std::string key = (forPreview ? "p:" : "m:") + path.toStdString() +
        (skipFrames ? ":1" : ":0") +
        ":" + (wantVideo ? "v" : "-") + (wantAudio ? "a" : "-");
    std::lock_guard<std::mutex> lock(mutex);
    auto it = mediaByKey.find(key);
    if (it != mediaByKey.end()) {
        mediaLru.remove(key);
        mediaLru.push_front(key);
        return it->second;
    }
    auto created = createMedia(path, forPreview, skipFrames, -1, wantVideo, wantAudio);
    mediaByKey[key] = created;
    mediaLru.push_front(key);
    while (mediaByKey.size() > config.maxMediaInstances && !mediaLru.empty()) {
        const std::string evict = mediaLru.back();
        mediaLru.pop_back();
        mediaByKey.erase(evict);
    }
    return created;
}

void ConMediaPool::invalidatePath(const QString &path)
{
    std::lock_guard<std::mutex> lock(mutex);
    if (path.isEmpty()) {
        mediaByKey.clear();
        sharedFilesByPath.clear();
        mediaLru.clear();
        return;
    }
    for (auto it = mediaByKey.begin(); it != mediaByKey.end();) {
        if (it->first.find(path.toStdString()) != std::string::npos) {
            mediaLru.remove(it->first);
            it = mediaByKey.erase(it);
        } else {
            ++it;
        }
    }
    for (auto it = sharedFilesByPath.begin(); it != sharedFilesByPath.end();) {
        if (it->first.find(path.toStdString()) != std::string::npos) {
            it = sharedFilesByPath.erase(it);
        } else {
            ++it;
        }
    }
}

std::shared_ptr<IBurstcon> ConMediaPool::createMedia(const QString &path,
                                                     bool forPreview,
                                                     bool skipFrames,
                                                     BurstIndex decodeOrigin,
                                                     bool wantVideo,
                                                     bool wantAudio)
{
    Q_UNUSED(decodeOrigin);
    auto backend = std::make_shared<AvcppMediaDecoderBackend>(acquireSharedMediaFile(path),
                                                              skipFrames,
                                                              wantVideo,
                                                              wantAudio);
    std::shared_ptr<IBurstcon> media = std::make_shared<ConMedia>(path, backend);
    if (config.enableConMediaCache) {
        ConCacheConfig c = config.cacheConfig;
        if (forPreview && config.forcedPreviewQualityEnabled) {
            c.forcedVideoQualityEnabled = true;
            c.forcedVideoQuality = config.forcedPreviewQuality;
        }
        media = std::make_shared<ConCache>(media, path, c);
    }
    return media;
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

ConTrack::ConTrack(std::shared_ptr<ConMediaPool> mediaPool)
    : mediaPool(std::move(mediaPool))
{
}

void ConTrack::setEvents(const std::vector<ConTrackEvent> &events)
{
    std::vector<BoundEvent> bound;
    bound.reserve(events.size());
    for (const auto &event : events) {
        BoundEvent b;
        b.event = event;
        b.media = mediaPool ? mediaPool->acquire(event.sourcePath,
                                                 false,
                                                 false,
                                                 event.sourceIn,
                                                 event.hasVideo,
                                                 event.hasAudio)
                            : nullptr;
        bound.push_back(b);
    }
    std::sort(bound.begin(), bound.end(), [](const BoundEvent &a, const BoundEvent &b) {
        if (a.event.start != b.event.start) {
            return a.event.start < b.event.start;
        }
        return a.event.zIndex < b.event.zIndex;
    });
    std::lock_guard<std::mutex> lock(mutex);
    this->events = std::move(bound);
}

bool ConTrack::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    std::vector<BoundEvent> local;
    {
        std::lock_guard<std::mutex> lock(mutex);
        local = events;
    }
    if (type == BurstType::Audio) {
        std::vector<int64_t> mix;
        AudioBurst base;
        bool hasAny = false;
        for (const auto &bound : local) {
            const auto &event = bound.event;
            if (!event.hasAudio || !bound.media) {
                continue;
            }
            if (request.index < event.start || request.index >= event.start + event.duration) {
                continue;
            }
            AudioBurst burst;
            const BurstIndex localIndex = event.sourceIn + (request.index - event.start);
            if (!bound.media->audioBurst(BurstRequest{localIndex, request.quality, event.sourceIn}, burst)) {
                continue;
            }
            if (!hasAny) {
                base = burst;
                mix.assign(base.samples.size(), 0);
                hasAny = true;
            }
            if (mix.size() < burst.samples.size()) {
                mix.resize(burst.samples.size(), 0);
                base.samples.resize(burst.samples.size(), 0);
            }
            for (size_t i = 0; i < burst.samples.size(); ++i) {
                mix[i] += burst.samples[i];
            }
        }
        if (!hasAny) {
            return false;
        }
        for (size_t i = 0; i < mix.size(); ++i) {
            base.samples[i] = static_cast<int32_t>(std::clamp<int64_t>(
                mix[i],
                std::numeric_limits<int32_t>::min(),
                std::numeric_limits<int32_t>::max()));
        }
        base.index = request.index;
        out = base;
        return true;
    }

    bool found = false;
    VideoBurst best;
    for (const auto &bound : local) {
        const auto &event = bound.event;
        if (!event.hasVideo || !bound.media) {
            continue;
        }
        if (request.index < event.start || request.index >= event.start + event.duration) {
            continue;
        }
        VideoBurst burst;
        const BurstIndex localIndex = event.sourceIn + (request.index - event.start);
        if (!bound.media->videoBurst(BurstRequest{localIndex, request.quality, event.sourceIn}, burst)) {
            continue;
        }
        best = burst;
        found = true;
    }
    if (!found) {
        return false;
    }
    best.index = request.index;
    out = best;
    return true;
}

BurstOriginInfo ConTrack::originInfo() const
{
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto &bound : events) {
        if (bound.media) {
            return bound.media->originInfo();
        }
    }
    return BurstOriginInfo{};
}

void ConTrack::invalidate(const BurstRange &range)
{
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto &bound : events) {
        if (bound.media) {
            bound.media->invalidate(range);
        }
    }
}

void ConTrimmer::setTracks(const std::vector<std::shared_ptr<ConTrack>> &tracks)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->tracks = tracks;
}

bool ConTrimmer::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    std::vector<std::shared_ptr<ConTrack>> localTracks;
    {
        std::lock_guard<std::mutex> lock(mutex);
        localTracks = tracks;
    }
    if (type == BurstType::Audio) {
        std::vector<int64_t> mix;
        AudioBurst base;
        bool hasAny = false;
        for (const auto &track : localTracks) {
            if (!track) {
                continue;
            }
            AudioBurst burst;
            if (!track->audioBurst(request, burst)) {
                continue;
            }
            if (!hasAny) {
                base = burst;
                mix.assign(base.samples.size(), 0);
                hasAny = true;
            }
            if (mix.size() < burst.samples.size()) {
                mix.resize(burst.samples.size(), 0);
                base.samples.resize(burst.samples.size(), 0);
            }
            for (size_t i = 0; i < burst.samples.size(); ++i) {
                mix[i] += burst.samples[i];
            }
        }
        if (!hasAny) {
            return false;
        }
        for (size_t i = 0; i < mix.size(); ++i) {
            base.samples[i] = static_cast<int32_t>(std::clamp<int64_t>(
                mix[i],
                std::numeric_limits<int32_t>::min(),
                std::numeric_limits<int32_t>::max()));
        }
        base.index = request.index;
        out = base;
        return true;
    }

    std::vector<VideoBurst> videoLayers;
    videoLayers.reserve(localTracks.size());
    for (const auto &track : localTracks) {
        if (!track) {
            continue;
        }
        VideoBurst burst;
        if (!track->videoBurst(request, burst) || burst.frame.isNull()) {
            continue;
        }
        videoLayers.push_back(std::move(burst));
    }
    if (videoLayers.empty()) {
        return false;
    }
    if (videoLayers.size() == 1) {
        videoLayers[0].index = request.index;
        out = std::move(videoLayers[0]);
        return true;
    }
    QImage composed = videoLayers[0].frame.convertToFormat(QImage::Format_ARGB32);
    for (size_t i = 1; i < videoLayers.size(); ++i) {
        QPainter painter(&composed);
        painter.drawImage(composed.rect(), videoLayers[i].frame);
    }
    out = VideoBurst{request.index, BurstQuality{composed.width(), composed.height()}, std::move(composed)};
    return true;
}

BurstOriginInfo ConTrimmer::originInfo() const
{
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto &track : tracks) {
        if (track) {
            const BurstOriginInfo info = track->originInfo();
            if (info.sourceVideoQuality.width > 0 && info.sourceVideoQuality.height > 0) {
                return info;
            }
        }
    }
    return BurstOriginInfo{};
}

void ConTrimmer::invalidate(const BurstRange &range)
{
    std::vector<std::shared_ptr<ConTrack>> localTracks;
    {
        std::lock_guard<std::mutex> lock(mutex);
        localTracks = tracks;
    }
    for (const auto &track : localTracks) {
        if (track) {
            track->invalidate(range);
        }
    }
}

MonotonicPlaybackTimer::MonotonicPlaybackTimer(std::shared_ptr<IBurstcon> source)
    : source(std::move(source))
{
}

MonotonicPlaybackTimer::~MonotonicPlaybackTimer()
{
    stop();
}

void MonotonicPlaybackTimer::setSource(std::shared_ptr<IBurstcon> source)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->source = std::move(source);
}

void MonotonicPlaybackTimer::setMode(PlaybackMode mode, BurstIndex regionStart, BurstIndex regionEnd)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->mode = mode;
    this->regionStart = regionStart;
    this->regionEnd = regionEnd;
}

void MonotonicPlaybackTimer::setVideoRequestQuality(const BurstQuality &quality)
{
    std::lock_guard<std::mutex> lock(mutex);
    videoRequestQuality = quality;
}

void MonotonicPlaybackTimer::setPlaybackVideoFps(int fps)
{
    std::lock_guard<std::mutex> lock(mutex);
    playbackVideoFps = std::max(1, fps);
}

void MonotonicPlaybackTimer::setCallbacks(const Callbacks &callbacks)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->callbacks = callbacks;
}

void MonotonicPlaybackTimer::start(BurstIndex startIndex)
{
    stop();
    index.store(startIndex);
    running.store(true);
    timerThread = std::thread([this]() { runLoop(); });
}

void MonotonicPlaybackTimer::stop()
{
    running.store(false);
    if (timerThread.joinable()) {
        timerThread.join();
    }
}

bool MonotonicPlaybackTimer::isRunning() const
{
    return running.load();
}

BurstIndex MonotonicPlaybackTimer::currentIndex() const
{
    return index.load();
}

void MonotonicPlaybackTimer::runLoop()
{
    using Clock = std::chrono::steady_clock;
    auto loopStart = Clock::now();
    BurstIndex startIndex = index.load();
    BurstIndex lastVideoRequest = startIndex;
    BurstIndex lastAudioRequest = startIndex;
    {
        std::lock_guard<std::mutex> lock(mutex);
        const BurstIndex videoStep = outputVideoFrameStepTicks(playbackVideoFps);
        const BurstIndex audioStep =
            std::max<BurstIndex>(1, (kBurstTimebaseHz * static_cast<BurstIndex>(kDefaultAudioSamplesPerBurst)) / 48000);
        lastVideoRequest = startIndex - std::max<BurstIndex>(1, videoStep);
        lastAudioRequest = startIndex - std::max<BurstIndex>(1, audioStep);
    }

    while (running.load()) {
        const auto now = Clock::now();
        const auto elapsedUs = std::chrono::duration_cast<std::chrono::microseconds>(now - loopStart).count();
        BurstIndex current = startIndex + static_cast<BurstIndex>((elapsedUs * kBurstTimebaseHz) / 1000000);

        PlaybackMode localMode;
        BurstIndex localRegionStart = 0;
        BurstIndex localRegionEnd = 0;
        std::shared_ptr<IBurstcon> localSource;
        Callbacks localCallbacks;
        BurstQuality localVideoRequestQuality;
        BurstIndex videoStep;
        {
            std::lock_guard<std::mutex> lock(mutex);
            localMode = mode;
            localRegionStart = regionStart;
            localRegionEnd = regionEnd;
            localSource = source;
            localCallbacks = callbacks;
            localVideoRequestQuality = videoRequestQuality;
            videoStep = outputVideoFrameStepTicks(playbackVideoFps);
        }
        if (localMode == PlaybackMode::Region || localMode == PlaybackMode::RegionLoop) {
            if (localRegionEnd > localRegionStart) {
                if (current >= localRegionEnd) {
                    if (localMode == PlaybackMode::RegionLoop) {
                        const BurstIndex span = localRegionEnd - localRegionStart;
                        current = localRegionStart + ((current - localRegionStart) % span);
                        loopStart = now;
                        startIndex = current;
                        lastVideoRequest = current - std::max<BurstIndex>(1, videoStep);
                        const BurstIndex audioStepWrap = std::max<BurstIndex>(
                            1, (kBurstTimebaseHz * static_cast<BurstIndex>(kDefaultAudioSamplesPerBurst)) / 48000);
                        lastAudioRequest = current - std::max<BurstIndex>(1, audioStepWrap);
                    } else {
                        current = localRegionEnd;
                        running.store(false);
                    }
                } else if (current < localRegionStart) {
                    current = localRegionStart;
                }
            }
        }
        index.store(current);

        if (localSource) {
            const BurstIndex audioStep = std::max<BurstIndex>(1, (kBurstTimebaseHz * kDefaultAudioSamplesPerBurst) / 48000);

            // Feed audio before video so a slow video decode does not delay PCM to the device.
            BurstRequest audioRequest;
            audioRequest.index = (current / audioStep) * audioStep;
            if (current - lastAudioRequest >= audioStep) {
                AudioBurst ab;
                if (localSource->audioBurst(audioRequest, ab) && localCallbacks.onAudio) {
                    localCallbacks.onAudio(ab);
                }
                lastAudioRequest = audioRequest.index;
            }

            BurstRequest videoRequest;
            videoRequest.index = (current / videoStep) * videoStep;
            // Always use the project decode quality. Replacing 0×0 with sourceVideoQuality here
            // changes ConCache keys vs prefetch/warmup (which use decodeBurstQuality) and causes
            // constant cache misses, sparse video (~10–15 s picture updates), and audio starvation.
            videoRequest.quality = localVideoRequestQuality;
            if (current - lastVideoRequest >= videoStep) {
                VideoBurst vb;
                if (localSource->videoBurst(videoRequest, vb) && localCallbacks.onVideo) {
                    localCallbacks.onVideo(std::move(vb));
                }
                lastVideoRequest = videoRequest.index;
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(2));
    }
}

void MonotonicPlaybackTimer::preloadAhead(const std::shared_ptr<IBurstcon> &source,
                                          const BurstQuality &videoQuality,
                                          BurstIndex secondStart,
                                          BurstIndex videoStep,
                                          BurstIndex audioStep)
{
    if (!source) {
        return;
    }
    const BurstIndex end = secondStart + kBurstTimebaseHz;
    for (BurstIndex t = secondStart; t <= end; t += videoStep) {
        BurstRequest req;
        req.index = t;
        req.quality = videoQuality;
        VideoBurst out;
        (void)source->videoBurst(req, out);
    }
    for (BurstIndex t = secondStart; t <= end; t += audioStep) {
        BurstRequest req;
        req.index = t;
        AudioBurst out;
        (void)source->audioBurst(req, out);
    }
}

BurstIndex outputVideoFrameStepTicks(int outputFps)
{
    const int fps = std::max(1, outputFps);
    return std::max<BurstIndex>(1, kBurstTimebaseHz / static_cast<BurstIndex>(fps));
}

BurstIndex alignBurstIndexToOutputVideoFrame(int outputFps, BurstIndex index)
{
    const BurstIndex step = outputVideoFrameStepTicks(outputFps);
    const BurstIndex safe = std::max<BurstIndex>(0, index);
    return (safe / step) * step;
}

int64_t msToBurstIndex(int64_t ms)
{
    if (ms <= 0) {
        return 0;
    }
    return (ms * kBurstTimebaseHz) / 1000;
}

int64_t burstIndexToMs(BurstIndex index)
{
    if (index <= 0) {
        return 0;
    }
    return (index * 1000) / kBurstTimebaseHz;
}

BurstconMetrics metricsSnapshot()
{
    BurstconMetrics m;
    m.avcppOpenCalls = gAvcppOpenCalls.load();
    m.avcppPacketReads = gAvcppPacketReads.load();
    m.avcppVideoDecodeCalls = gAvcppVideoDecodeCalls.load();
    m.avcppAudioDecodeCalls = gAvcppAudioDecodeCalls.load();
    m.cacheHits = gCacheHits.load();
    m.cacheMisses = gCacheMisses.load();
    m.cacheEntries = static_cast<uint64_t>(std::max<int64_t>(0, gCacheEntries.load()));
    return m;
}

void resetMetrics()
{
    gAvcppOpenCalls.store(0);
    gAvcppPacketReads.store(0);
    gAvcppVideoDecodeCalls.store(0);
    gAvcppAudioDecodeCalls.store(0);
    gCacheHits.store(0);
    gCacheMisses.store(0);
    gCacheEntries.store(0);
}

} // namespace burstcon
