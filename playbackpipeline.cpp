#include "playbackpipeline.h"

#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
#include <QAudio>
#include <QAudioSink>
#include <QMediaDevices>
#else
#include <QAudio>
#include <QAudioOutput>
#endif
#include <QAudioFormat>
#include <QFileInfo>
#include <QFont>
#include <QIODevice>
#include <QPainter>
#include <QJsonDocument>

#include <algorithm>
#include <chrono>
#include <cstring>
#include <limits>
#include <thread>
#include <vector>

#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
class PlaybackPipeline::PreviewPcmIODevice final : public QIODevice
{
public:
    explicit PreviewPcmIODevice(PlaybackPipeline *owner)
        : QIODevice()
        , pipe(owner)
    {
        open(QIODevice::ReadOnly);
    }

    bool isSequential() const override { return true; }

protected:
    qint64 readData(char *data, qint64 maxSize) override
    {
        if (!pipe || !data || maxSize <= 0) {
            return 0;
        }
        return pipe->readAudioPcmForDevice(data, maxSize);
    }

    qint64 writeData(const char *, qint64) override { return -1; }

private:
    PlaybackPipeline *pipe = nullptr;
};
#endif

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr int kDefaultAudioSamplesPerBurst = 1024;
constexpr qint64 kVideoCoverageBucketMs = 33;

qint64 videoCoverageBucketCenterMs(qint64 tMs)
{
    const qint64 step = std::max<qint64>(1, kVideoCoverageBucketMs);
    return ((tMs + step / 2) / step) * step;
}
constexpr qint64 kAudioCoverageBucketMs = 20;
/// Upper bound for `prefetchEndMs` in `update` (before per-mode min). Keep in sync with idle
/// lookahead so `min(t + kPrefetchAheadMs, t + kIdleScrubPrefetchAheadMs)` is not accidentally 1 s.
constexpr qint64 kPrefetchAheadMs = 10000;
constexpr qint64 kIdleScrubPrefetchAheadMs = 10000;
/// While playing, prefetch this far ahead of the playhead (decode + ConCache), capped by `prefetchEndMs`.
constexpr qint64 kPrefetchDuringPlayAheadMs = 5000;
/// How often `PlaybackPipeline::update` may extend the rolling prefetch window while playing.
constexpr qint64 kPrefetchPlayRescheduleMs = 400;
/// For prefetch/warmup spans up to this length, step video at output FPS so ConCache keys match playback.
/// Wider sparse steps add different indices than the timer, evict playback frames in LRU and break short loops.
constexpr qint64 kPrefetchVideoDenseMaxSpanMs = 6000;
constexpr qint64 kMaxCoverageProbeSteps = 40000;

qint64 coverageTimeStepMs(qint64 spanMs, qint64 baseStepMs)
{
    const qint64 safeBase = std::max<qint64>(1, baseStepMs);
    if (spanMs <= 0) {
        return safeBase;
    }
    qint64 step = safeBase;
    const qint64 buckets = spanMs / step;
    if (buckets > kMaxCoverageProbeSteps) {
        step = spanMs / kMaxCoverageProbeSteps;
        step = std::max(step, safeBase);
    }
    return std::max<qint64>(1, step);
}

burstcon::BurstIndex msToBurstIndexCeil(qint64 ms)
{
    const qint64 safe = std::max<qint64>(1, ms);
    return static_cast<burstcon::BurstIndex>((safe * burstcon::kBurstTimebaseHz + 999) / 1000);
}

burstcon::BurstQuality decodeBurstQuality(const ProjectSettings &settings)
{
    const QSize q = settings.cacheResolution();
    if (settings.scaleVideoAtDisplayTime) {
        // YUV→RGB in swscale runs at this size; keep it bounded. QPainter then scales to the widget.
        // BurstQuality {0,0} would expand to full coded size in ConCache and costs much more in
        // ff_yuv_420_rgb24_* than scaling inside the same swscale step to cacheResolution().
        return burstcon::BurstQuality{q.width(), q.height()};
    }
    // Full-resolution RGB decode (heavy); still drawn scaled to the widget in the preview.
    return burstcon::BurstQuality{0, 0};
}

/// Hard upper bound for **realtime** preview / playback / prefetch in this pipeline only.
/// Export and draft render elsewhere may use full `decodeBurstQuality`; FHD/4K full-RGB here stalls the machine.
burstcon::BurstQuality decodeRealtimePipelineVideoQuality(const ProjectSettings &settings)
{
    QSize q;
    if (settings.scaleVideoAtDisplayTime) {
        q = settings.cacheResolution();
    } else {
        // Match old "full decode" intent for the panel, but never ask the decoder for >1080p-class RGB.
        q = QSize(1920, 1080);
    }
    const QSize cap(1920, 1080);
    if (q.isEmpty() || q.width() < 1 || q.height() < 1) {
        q = QSize(960, 540);
    }
    if (q.width() > cap.width() || q.height() > cap.height()) {
        q = q.scaled(cap, Qt::KeepAspectRatio);
    }
    q.setWidth(std::max(320, q.width()));
    q.setHeight(std::max(180, q.height()));
    return burstcon::BurstQuality{q.width(), q.height()};
}

QVector<QPair<double, double>> segmentsFromBucketsAlongSpan(const std::unordered_set<qint64> &buckets,
                                                            qint64 spanStartMs,
                                                            qint64 spanEndMs)
{
    QVector<QPair<double, double>> segs;
    if (spanEndMs <= spanStartMs) {
        return segs;
    }
    const qint64 span = spanEndMs - spanStartMs;
    bool inRun = false;
    qint64 runStart = spanStartMs;
    const qint64 stepMs = coverageTimeStepMs(span, kVideoCoverageBucketMs);
    for (qint64 t = spanStartMs; t < spanEndMs; t += stepMs) {
        const qint64 bucket = videoCoverageBucketCenterMs(t);
        const bool hit = buckets.find(bucket) != buckets.end();
        if (hit && !inRun) {
            inRun = true;
            runStart = t;
        } else if (!hit && inRun) {
            const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
            const double b = static_cast<double>(t - spanStartMs) / static_cast<double>(span);
            segs.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
            inRun = false;
        }
    }
    if (inRun) {
        const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
        segs.push_back({std::clamp(a, 0.0, 1.0), 1.0});
    }
    return segs;
}

static qint64 audioCoverageBucketMs(qint64 tMs)
{
    return (tMs / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
}

QVector<QPair<double, double>> segmentsFromBucketsAlongSpanAudio(const std::unordered_set<qint64> &buckets,
                                                                 qint64 spanStartMs,
                                                                 qint64 spanEndMs)
{
    QVector<QPair<double, double>> segs;
    if (spanEndMs <= spanStartMs) {
        return segs;
    }
    const qint64 span = spanEndMs - spanStartMs;
    bool inRun = false;
    qint64 runStart = spanStartMs;
    const qint64 stepMs = coverageTimeStepMs(span, kAudioCoverageBucketMs);
    for (qint64 t = spanStartMs; t < spanEndMs; t += stepMs) {
        const qint64 bucket = audioCoverageBucketMs(t);
        const bool hit = buckets.find(bucket) != buckets.end();
        if (hit && !inRun) {
            inRun = true;
            runStart = t;
        } else if (!hit && inRun) {
            const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
            const double b = static_cast<double>(t - spanStartMs) / static_cast<double>(span);
            segs.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
            inRun = false;
        }
    }
    if (inRun) {
        const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
        segs.push_back({std::clamp(a, 0.0, 1.0), 1.0});
    }
    return segs;
}

QVector<QPair<double, double>> segmentsFromBucketsAlongSpanAudioForward(const std::unordered_set<qint64> &buckets,
                                                                        qint64 spanStartMs,
                                                                        qint64 spanEndMs,
                                                                        qint64 forwardAnchorMs)
{
    if (forwardAnchorMs < 0) {
        return segmentsFromBucketsAlongSpanAudio(buckets, spanStartMs, spanEndMs);
    }
    QVector<QPair<double, double>> segs;
    if (spanEndMs <= spanStartMs) {
        return segs;
    }
    const qint64 span = spanEndMs - spanStartMs;
    const qint64 grid = std::max<qint64>(1, kAudioCoverageBucketMs);
    const qint64 start = std::max(spanStartMs, (forwardAnchorMs / grid) * grid);
    if (spanEndMs <= start) {
        return segs;
    }
    bool inRun = false;
    qint64 runStart = start;
    const qint64 stepMs = coverageTimeStepMs(spanEndMs - start, kAudioCoverageBucketMs);
    for (qint64 t = start; t < spanEndMs; t += stepMs) {
        const qint64 bucket = audioCoverageBucketMs(t);
        const bool hit = buckets.find(bucket) != buckets.end();
        if (hit && !inRun) {
            inRun = true;
            runStart = t;
        } else if (!hit && inRun) {
            const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
            const double b = static_cast<double>(t - spanStartMs) / static_cast<double>(span);
            segs.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
            inRun = false;
        }
    }
    if (inRun) {
        const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
        segs.push_back({std::clamp(a, 0.0, 1.0), 1.0});
    }
    return segs;
}

/// Project-ruler strip: only show coverage at/after `forwardAnchorMs` (lookahead from playhead).
QVector<QPair<double, double>> segmentsFromBucketsAlongSpanForward(const std::unordered_set<qint64> &buckets,
                                                                   qint64 spanStartMs,
                                                                   qint64 spanEndMs,
                                                                   qint64 forwardAnchorMs)
{
    if (forwardAnchorMs < 0) {
        return segmentsFromBucketsAlongSpan(buckets, spanStartMs, spanEndMs);
    }
    QVector<QPair<double, double>> segs;
    if (spanEndMs <= spanStartMs) {
        return segs;
    }
    const qint64 span = spanEndMs - spanStartMs;
    const qint64 grid = std::max<qint64>(1, kVideoCoverageBucketMs);
    const qint64 start = std::max(spanStartMs, (forwardAnchorMs / grid) * grid);
    if (spanEndMs <= start) {
        return segs;
    }
    bool inRun = false;
    qint64 runStart = start;
    const qint64 stepMs = coverageTimeStepMs(spanEndMs - start, kVideoCoverageBucketMs);
    for (qint64 t = start; t < spanEndMs; t += stepMs) {
        const qint64 bucket = videoCoverageBucketCenterMs(t);
        const bool hit = buckets.find(bucket) != buckets.end();
        if (hit && !inRun) {
            inRun = true;
            runStart = t;
        } else if (!hit && inRun) {
            const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
            const double b = static_cast<double>(t - spanStartMs) / static_cast<double>(span);
            segs.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
            inRun = false;
        }
    }
    if (inRun) {
        const double a = static_cast<double>(runStart - spanStartMs) / static_cast<double>(span);
        segs.push_back({std::clamp(a, 0.0, 1.0), 1.0});
    }
    return segs;
}

void insertVideoCoverageForCompositionMs(std::unordered_set<qint64> &buckets, qint64 composeMs, int fps)
{
    const qint64 frameMs = std::max<qint64>(1, 1000 / std::max(1, fps));
    const qint64 step = std::max<qint64>(1, kVideoCoverageBucketMs / 2);
    const qint64 tail = composeMs + 2 * frameMs + kVideoCoverageBucketMs * 3;
    for (qint64 u = composeMs - kVideoCoverageBucketMs * 2; u <= tail; u += step) {
        buckets.insert(videoCoverageBucketCenterMs(u));
    }
}

QImage makeBlackFrame(int w, int h)
{
    QImage img(std::max(320, w), std::max(180, h), QImage::Format_RGB32);
    img.fill(Qt::black);
    return img;
}

QImage makeDraftFallbackImage(int w, int h, const QString &line1, const QString &line2 = {})
{
    QImage img(std::max(320, w), std::max(180, h), QImage::Format_RGB32);
    img.fill(QColor(10, 10, 12));
    QPainter p(&img);
    p.setPen(QColor(210, 215, 225));
    QFont f = p.font();
    f.setPointSize(13);
    p.setFont(f);
    QString body = line1;
    if (!line2.isEmpty()) {
        body += QLatin1Char('\n');
        body += line2;
    }
    p.drawText(img.rect().adjusted(24, 24, -24, -24), Qt::AlignCenter | Qt::TextWordWrap, body);
    return img;
}

QString activeVideoClipPathAtMs(qint64 tMs, const std::vector<PlaybackPipeline::Clip> &clips)
{
    for (const auto &c : clips) {
        if (tMs >= c.startMs && tMs < c.startMs + std::max<qint64>(1, c.durationMs)) {
            return c.sourcePath;
        }
    }
    return {};
}

bool compositionHasAudioAtMs(qint64 tMs, const std::vector<PlaybackPipeline::Clip> &clips)
{
    for (const auto &c : clips) {
        if (tMs >= c.startMs && tMs < c.startMs + std::max<qint64>(1, c.durationMs)) {
            return true;
        }
    }
    return false;
}

constexpr qint64 kRegionHeadWindowMs = 1500;

/// Head strip = sliding cache on the composition timeline, restricted to [regionStart, regionStart+W).
QVector<QPair<double, double>> headSegmentsOnView(const std::unordered_set<qint64> &slidingBuckets,
                                                   qint64 viewStartMs,
                                                   qint64 viewEndMs,
                                                   qint64 regionStartMs,
                                                   qint64 regionEndMs)
{
    if (viewEndMs <= viewStartMs || regionEndMs <= regionStartMs) {
        return {};
    }
    const qint64 headEnd = std::min(regionEndMs, regionStartMs + kRegionHeadWindowMs);
    const qint64 overlapStart = std::max(viewStartMs, regionStartMs);
    const qint64 overlapEnd = std::min(viewEndMs, headEnd);
    if (overlapEnd <= overlapStart) {
        return {};
    }
    const auto inner = segmentsFromBucketsAlongSpan(slidingBuckets, overlapStart, overlapEnd);
    const double innerSpan = static_cast<double>(overlapEnd - overlapStart);
    const double outerSpan = static_cast<double>(viewEndMs - viewStartMs);
    if (innerSpan <= 0.0 || outerSpan <= 0.0) {
        return {};
    }
    QVector<QPair<double, double>> out;
    for (const auto &seg : inner) {
        const double t0 = static_cast<double>(overlapStart) + seg.first * innerSpan;
        const double t1 = static_cast<double>(overlapStart) + seg.second * innerSpan;
        const double a = (t0 - static_cast<double>(viewStartMs)) / outerSpan;
        const double b = (t1 - static_cast<double>(viewStartMs)) / outerSpan;
        if (b > a) {
            out.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
        }
    }
    return out;
}

QVector<QPair<double, double>> headSegmentsOnClip(const std::unordered_set<qint64> &slidingBuckets,
                                                   qint64 clipStartMs,
                                                   qint64 clipDurationMs,
                                                   qint64 regionStartMs,
                                                   qint64 regionEndMs)
{
    QVector<QPair<double, double>> empty;
    if (clipDurationMs <= 0 || regionEndMs <= regionStartMs) {
        return empty;
    }
    const qint64 clipEnd = clipStartMs + clipDurationMs;
    const qint64 headEnd = std::min(regionEndMs, regionStartMs + kRegionHeadWindowMs);
    const qint64 overlapStart = std::max(clipStartMs, regionStartMs);
    const qint64 overlapEnd = std::min(clipEnd, headEnd);
    if (overlapEnd <= overlapStart) {
        return empty;
    }
    const auto inner = segmentsFromBucketsAlongSpan(slidingBuckets, overlapStart, overlapEnd);
    const double innerSpan = static_cast<double>(overlapEnd - overlapStart);
    const double outerSpan = static_cast<double>(clipDurationMs);
    if (innerSpan <= 0.0 || outerSpan <= 0.0) {
        return empty;
    }
    QVector<QPair<double, double>> out;
    for (const auto &seg : inner) {
        const double t0 = static_cast<double>(overlapStart) + seg.first * innerSpan;
        const double t1 = static_cast<double>(overlapStart) + seg.second * innerSpan;
        const double a = (t0 - static_cast<double>(clipStartMs)) / outerSpan;
        const double b = (t1 - static_cast<double>(clipStartMs)) / outerSpan;
        if (b > a) {
            out.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
        }
    }
    return out;
}

} // namespace

PlaybackPipeline::PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady)
    : onVideoFrameReady(std::move(onVideoFrameReady))
{
    burstcon::ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = true;
    // Total LRU slots (audio + video). Keep bounded so one path cannot pin unbounded RAM.
    poolCfg.cacheConfig.maxEntries = 384*384;
    poolCfg.cacheConfig.maxAudioEntries = 192*192;
    poolCfg.cacheConfig.maxVideoEntries = 256*256;
    // ConCache byte ceiling for video QImages (LRU also evicts by maxVideoEntries / maxEntries).
    poolCfg.cacheConfig.maxVideoBytes = 4ULL * 1024ULL * 1024ULL * 1024ULL;
    poolCfg.cacheConfig.preloadEnabled = true;
    poolCfg.cacheConfig.preloadAheadTicks = burstcon::kBurstTimebaseHz;
    poolCfg.maxMediaInstances = 16*16;
    poolCfg.cacheConfig.onVideoFrameCacheChanged =
        [this](bool inserted, const QString &mediaPath, burstcon::BurstIndex mediaBurstIndex, const burstcon::BurstQuality &q) {
            onVideoFrameCacheKeyChanged(inserted, mediaPath, mediaBurstIndex, q);
        };
    poolCfg.cacheConfig.onAudioBurstCacheChanged =
        [this](bool inserted, const QString &mediaPath, burstcon::BurstIndex mediaBurstIndex) {
            onAudioBurstCacheKeyChanged(inserted, mediaPath, mediaBurstIndex);
        };
    mediaPool = std::make_shared<burstcon::ConMediaPool>(poolCfg);
    videoTrack = std::make_shared<burstcon::ConTrack>(mediaPool);
    audioTrack = std::make_shared<burstcon::ConTrack>(mediaPool);
    trimmer = std::make_shared<burstcon::ConTrimmer>();
    trimmer->setTracks({videoTrack, audioTrack});
    root = trimmer;
    timer = std::make_unique<burstcon::MonotonicPlaybackTimer>(root);
    draftWorker = std::thread([this]() { draftWorkerLoop(); });
    prefetchWorker = std::thread([this]() { prefetchWorkerLoop(); });
}

PlaybackPipeline::~PlaybackPipeline()
{
    stop();
    {
        std::lock_guard<std::mutex> lock(draftMutex);
        stopDraftWorker = true;
        draftJobPending = false;
    }
    draftCv.notify_all();
    if (draftWorker.joinable()) {
        draftWorker.join();
    }
    {
        std::lock_guard<std::mutex> lock(prefetchMutex);
        stopPrefetchWorker = true;
        prefetchJobPending = false;
    }
    prefetchCv.notify_all();
    if (prefetchWorker.joinable()) {
        prefetchWorker.join();
    }
}

void PlaybackPipeline::preparePlaybackAt(qint64 startTimelineMs)
{
    running.store(true);
    latestDraftSerial.fetch_add(1);
    timelineMs.store(startTimelineMs);
    ensureAudioOutput();
    burstcon::MonotonicPlaybackTimer::Callbacks callbacks;
    callbacks.onVideo = [this](burstcon::VideoBurst burst) {
        const qint64 tMs = burstcon::burstIndexToMs(burst.index);
        timelineMs.store(tMs);
        lastPlaybackIndex.store(burst.index);
        int fps = 30;
        {
            std::lock_guard<std::mutex> lk(mutex);
            fps = std::max(1, projectSettings.fps);
        }
        {
            std::lock_guard<std::mutex> cov(coverageMutex);
            insertVideoCoverageForCompositionMs(timelineVideoSlidingBuckets, tMs, fps);
            const qint64 pruneBefore = tMs - 180000;
            for (auto it = timelineVideoSlidingBuckets.begin(); it != timelineVideoSlidingBuckets.end();) {
                if (*it < pruneBefore) {
                    it = timelineVideoSlidingBuckets.erase(it);
                } else {
                    ++it;
                }
            }
        }
        if (onVideoFrameReady) {
            if (!burst.frame.isNull()) {
                onVideoFrameReady(std::move(burst.frame));
                composedFrames.fetch_add(1);
            } else {
                QImage black(640, 360, QImage::Format_RGB32);
                black.fill(Qt::black);
                onVideoFrameReady(std::move(black));
            }
        }
    };
    callbacks.onAudio = [this](const burstcon::AudioBurst &burst) {
        const qint64 tMs = burstcon::burstIndexToMs(burst.index);
        {
            std::lock_guard<std::mutex> lock(coverageMutex);
            audioCoverageBuckets.insert((tMs / kAudioCoverageBucketMs) * kAudioCoverageBucketMs);
        }
        pushAudioBurst(burst);
    };
    const burstcon::BurstQuality q = decodeRealtimePipelineVideoQuality(projectSettings);
    {
        std::lock_guard<std::mutex> lock(mutex);
        timer->setVideoRequestQuality(q);
        timer->setPlaybackVideoFps(std::max(1, projectSettings.fps));
        timer->setCallbacks(callbacks);
    }
    lastVideoRequestIndex.store(-1);
    lastAudioRequestIndex.store(-1);
    lastPrefetchAnchorMs.store(startTimelineMs);
}

void PlaybackPipeline::warmPlaybackHeadBlocking(qint64 startTimelineMs, qint64 headMs)
{
    std::shared_ptr<burstcon::IBurstcon> localRoot;
    burstcon::BurstQuality quality;
    int fps = 30;
    qint64 warmEndMs = startTimelineMs + headMs;
    burstcon::PlaybackMode pbMode = burstcon::PlaybackMode::Free;
    qint64 regStartMs = 0;
    qint64 regEndMs = 0;
    {
        std::lock_guard<std::mutex> lock(mutex);
        localRoot = root;
        quality = decodeRealtimePipelineVideoQuality(projectSettings);
        fps = std::max(1, projectSettings.fps);
        pbMode = playbackMode;
        regStartMs = playbackRegionStartMs;
        regEndMs = playbackRegionEndMs;
        if ((playbackMode == burstcon::PlaybackMode::Region || playbackMode == burstcon::PlaybackMode::RegionLoop) &&
            playbackRegionEndMs > playbackRegionStartMs) {
            warmEndMs = std::min(warmEndMs, playbackRegionEndMs);
        }
    }
    if (!localRoot || warmEndMs <= startTimelineMs) {
        return;
    }
    const burstcon::BurstIndex videoStep = burstcon::outputVideoFrameStepTicks(fps);
    const qint64 warmSpanMs = warmEndMs - startTimelineMs;
    const burstcon::BurstIndex warmVideoStep =
        warmSpanMs <= kPrefetchVideoDenseMaxSpanMs
            ? videoStep
            : std::max(videoStep, burstcon::kBurstTimebaseHz / 10);
    const burstcon::BurstIndex audioStep = std::max<burstcon::BurstIndex>(
        1, (burstcon::kBurstTimebaseHz * static_cast<burstcon::BurstIndex>(kDefaultAudioSamplesPerBurst)) / kAudioSampleRate);
    const burstcon::BurstIndex startIdx = burstcon::msToBurstIndex(startTimelineMs);
    const burstcon::BurstIndex alignedStart =
        warmSpanMs <= kPrefetchVideoDenseMaxSpanMs
            ? burstcon::alignBurstIndexToOutputVideoFrame(fps, startIdx)
            : (startIdx / warmVideoStep) * warmVideoStep;
    const burstcon::BurstIndex endIdx = msToBurstIndexCeil(warmEndMs);
    for (burstcon::BurstIndex idx = alignedStart; idx <= endIdx; idx += warmVideoStep) {
        if (!running.load()) {
            return;
        }
        burstcon::BurstRequest req;
        req.index = idx;
        req.quality = quality;
        burstcon::VideoBurst vb;
        if (localRoot->videoBurst(req, vb) && !vb.frame.isNull()) {
            const qint64 tMs = burstcon::burstIndexToMs(idx);
            if ((pbMode == burstcon::PlaybackMode::Region || pbMode == burstcon::PlaybackMode::RegionLoop) &&
                regEndMs > regStartMs) {
                const qint64 headEnd = std::min(regEndMs, regStartMs + kRegionHeadWindowMs);
                if (tMs >= regStartMs && tMs < headEnd) {
                    std::lock_guard<std::mutex> cov(coverageMutex);
                    timelineVideoRegionHeadBuckets.insert(videoCoverageBucketCenterMs(tMs));
                }
            }
        }
    }
    const burstcon::BurstIndex alignedAudioStart = (startIdx / audioStep) * audioStep;
    for (burstcon::BurstIndex idx = alignedAudioStart; idx <= endIdx; idx += audioStep) {
        if (!running.load()) {
            return;
        }
        burstcon::BurstRequest req;
        req.index = idx;
        burstcon::AudioBurst ab;
        if (localRoot->audioBurst(req, ab)) {
            std::lock_guard<std::mutex> lock(coverageMutex);
            const qint64 tMs = burstcon::burstIndexToMs(idx);
            const qint64 bucket = (tMs / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
            audioCoverageBuckets.insert(bucket);
        }
    }
}

void PlaybackPipeline::beginClockAndPrefetch(qint64 startTimelineMs)
{
    if (timer) {
        timer->start(burstcon::msToBurstIndex(startTimelineMs));
    }
    qint64 prefetchEndMs = startTimelineMs + std::min(kPrefetchAheadMs, kIdleScrubPrefetchAheadMs);
    {
        std::lock_guard<std::mutex> lock(mutex);
        if ((playbackMode == burstcon::PlaybackMode::Region || playbackMode == burstcon::PlaybackMode::RegionLoop) &&
            playbackRegionEndMs > playbackRegionStartMs) {
            prefetchEndMs = std::min(prefetchEndMs, playbackRegionEndMs);
        }
    }
    if (prefetchEndMs > startTimelineMs &&
        !isPlaybackHeadWarm(startTimelineMs, prefetchEndMs - startTimelineMs)) {
        requestPrefetchAsync(startTimelineMs, prefetchEndMs, true);
    }
}

bool PlaybackPipeline::isPlaybackHeadWarm(qint64 startMs, qint64 headMs) const
{
    if (headMs <= 0) {
        return true;
    }
    qint64 endMs = startMs + headMs;
    std::vector<Clip> videoClips;
    std::vector<Clip> audioClips;
    int fps = 30;
    {
        std::lock_guard<std::mutex> lock(mutex);
        if ((playbackMode == burstcon::PlaybackMode::Region || playbackMode == burstcon::PlaybackMode::RegionLoop) &&
            playbackRegionEndMs > playbackRegionStartMs) {
            endMs = std::min(endMs, playbackRegionEndMs);
        }
        fps = std::max(1, projectSettings.fps);
        videoClips = lastVideoClips;
        audioClips = lastAudioClips;
    }
    if (endMs <= startMs) {
        return true;
    }
    // Without any video clips, sliding video buckets are never populated; requiring them here kept
    // the head "cold" forever (idle prefetch/draft churn). Same idea as skipping audio probes when
    // there is no audio in the graph.
    if (videoClips.empty() && audioClips.empty()) {
        return true;
    }
    constexpr qint64 kProbeStepMs = 400;
    std::lock_guard<std::mutex> lock(coverageMutex);
    // Only require coverage where composition actually has video/audio. The old logic required
    // video buckets on the entire probe grid whenever *any* video clip existed in the project,
    // so gaps on the timeline were never "warm" and every play-pause retriggered prefetch/warmup
    // (avcpp spikes) even with a full ConCache.
    for (qint64 t = startMs; t < endMs; t += kProbeStepMs) {
        if (compositionHasAudioAtMs(t, audioClips)) {
            const qint64 aBucket = (t / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
            if (audioCoverageBuckets.find(aBucket) == audioCoverageBuckets.end()) {
                return false;
            }
        }
    }
    // Video: walk output-aligned frame times inside each clip ∩ [start,end) so we match the
    // buckets written during playback (coarse 400 ms grid misses sub‑400 ms clips).
    const burstcon::BurstIndex videoStepTicks = burstcon::outputVideoFrameStepTicks(fps);
    for (const auto &clip : videoClips) {
        const qint64 clipEnd = clip.startMs + std::max<qint64>(1, clip.durationMs);
        const qint64 overlapLo = std::max(startMs, clip.startMs);
        const qint64 overlapHi = std::min(endMs, clipEnd);
        if (overlapHi <= overlapLo) {
            continue;
        }
        burstcon::BurstIndex idxLo =
            burstcon::alignBurstIndexToOutputVideoFrame(fps, burstcon::msToBurstIndex(overlapLo));
        burstcon::BurstIndex idxHi =
            burstcon::alignBurstIndexToOutputVideoFrame(fps, burstcon::msToBurstIndex(overlapHi - 1));
        if (idxLo > idxHi) {
            std::swap(idxLo, idxHi);
        }
        bool sawProbeInsideOverlap = false;
        for (burstcon::BurstIndex idx = idxLo; idx <= idxHi; idx += videoStepTicks) {
            const qint64 tMs = burstcon::burstIndexToMs(idx);
            if (tMs < overlapLo || tMs >= overlapHi) {
                continue;
            }
            sawProbeInsideOverlap = true;
            const qint64 bucket = videoCoverageBucketCenterMs(tMs);
            if (timelineVideoSlidingBuckets.find(bucket) == timelineVideoSlidingBuckets.end()) {
                return false;
            }
        }
        // If the clip overlaps the window but every aligned frame lands outside the clip (short
        // event vs output grid), we must not report "warm" without checking any bucket — that
        // skipped draft/warmup and left the preview black.
        if (!sawProbeInsideOverlap) {
            return false;
        }
    }
    for (const auto &clip : audioClips) {
        const qint64 cEnd = clip.startMs + std::max<qint64>(1, clip.durationMs);
        for (qint64 edge : {clip.startMs, cEnd - 1}) {
            if (edge < startMs || edge >= endMs) {
                continue;
            }
            const qint64 aBucket = (edge / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
            if (audioCoverageBuckets.find(aBucket) == audioCoverageBuckets.end()) {
                return false;
            }
        }
    }
    return true;
}

bool PlaybackPipeline::isScrubFrameWarm(qint64 tMs) const
{
    std::vector<Clip> videoClips;
    std::vector<Clip> audioClips;
    int fps = 30;
    {
        std::lock_guard<std::mutex> lock(mutex);
        fps = std::max(1, projectSettings.fps);
        videoClips = lastVideoClips;
        audioClips = lastAudioClips;
    }
    const bool wantAudio = compositionHasAudioAtMs(tMs, audioClips);
    const burstcon::BurstIndex alignedIdx =
        burstcon::alignBurstIndexToOutputVideoFrame(fps, burstcon::msToBurstIndex(tMs));
    const qint64 sampleMs = burstcon::burstIndexToMs(alignedIdx);
    const bool wantVideo = !activeVideoClipPathAtMs(sampleMs, videoClips).isEmpty();
    if (!wantVideo && !wantAudio) {
        return true;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    if (wantVideo) {
        const qint64 bucket = videoCoverageBucketCenterMs(sampleMs);
        if (timelineVideoSlidingBuckets.find(bucket) == timelineVideoSlidingBuckets.end()) {
            return false;
        }
    }
    if (wantAudio) {
        const qint64 aBucket = (tMs / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
        if (audioCoverageBuckets.find(aBucket) == audioCoverageBuckets.end()) {
            return false;
        }
    }
    return true;
}

void PlaybackPipeline::start(qint64 startTimelineMs)
{
    preparePlaybackAt(startTimelineMs);
    warmPlaybackHeadBlocking(startTimelineMs, 5000);
    beginClockAndPrefetch(startTimelineMs);
}

void PlaybackPipeline::stop()
{
    running.store(false);
    scrubPlaybackActive.store(false);
    latestDraftSerial.fetch_add(1);
    auto *localTimer = timer.get();
    if (localTimer) {
        localTimer->stop();
    }
    shutdownAudioOutput();
    lastVideoRequestIndex.store(-1);
    lastAudioRequestIndex.store(-1);
    lastPrefetchAnchorMs.store(-1);
}

bool PlaybackPipeline::isRunning() const
{
    return timer && timer->isRunning();
}

void PlaybackPipeline::update(qint64 tMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips)
{
    qint64 prefetchEndMs = tMs + kPrefetchAheadMs;
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (!clipsEqual(videoClips, lastVideoClips) || !clipsEqual(audioClips, lastAudioClips)) {
            rebuildGraph(videoClips, audioClips);
            lastVideoClips = videoClips;
            lastAudioClips = audioClips;
        }
        if ((playbackMode == burstcon::PlaybackMode::Region || playbackMode == burstcon::PlaybackMode::RegionLoop) &&
            playbackRegionEndMs > playbackRegionStartMs) {
            prefetchEndMs = std::min(prefetchEndMs, playbackRegionEndMs);
        }
    }
    if (!running.load()) {
        timelineMs.store(tMs);
        lastPrefetchAnchorMs.store(tMs);
        const qint64 idlePrefetchEnd = std::min(prefetchEndMs, tMs + kIdleScrubPrefetchAheadMs);
        const qint64 idleSpan = std::max<qint64>(0, idlePrefetchEnd - tMs);
        const bool headWarm = idleSpan > 0 && isPlaybackHeadWarm(tMs, idleSpan);
        // Do not tie idle preview to the full lookahead window: that window can be "warm" from
        // cache elsewhere while the aligned frame under the cursor still has no bucket, so draft
        // was skipped and one clip showed stripes but black preview.
        if (onVideoFrameReady && !scrubPlaybackActive.load()) {
            requestIdlePreviewAsync(tMs, videoClips);
        }
        if (idleSpan > 0 && !headWarm) {
            requestPrefetchAsync(tMs, idlePrefetchEnd, true);
        }
    } else {
        const qint64 anchor = lastPrefetchAnchorMs.load();
        if (tMs > anchor + kPrefetchPlayRescheduleMs) {
            lastPrefetchAnchorMs.store(tMs);
            const qint64 playPrefetchEnd = std::min(prefetchEndMs, tMs + kPrefetchDuringPlayAheadMs);
            const qint64 span = playPrefetchEnd - tMs;
            if (span > 0 && !isPlaybackHeadWarm(tMs, span)) {
                requestPrefetchAsync(tMs, playPrefetchEnd, true);
            }
        }
    }
}

void PlaybackPipeline::setPlaybackMode(burstcon::PlaybackMode mode, qint64 regionStartMs, qint64 regionEndMs)
{
    qint64 regionPrefetchStart = 0;
    qint64 regionPrefetchEnd = 0;
    bool wantRegionIdlePrefetch = false;
    bool clearRegionHeadBuckets = false;
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (playbackMode != mode) {
            clearRegionHeadBuckets = true;
        } else if (mode != burstcon::PlaybackMode::Free &&
                   (playbackRegionStartMs != regionStartMs || playbackRegionEndMs != regionEndMs)) {
            clearRegionHeadBuckets = true;
        }
        playbackMode = mode;
        if (mode == burstcon::PlaybackMode::Free) {
            playbackRegionStartMs = 0;
            playbackRegionEndMs = 0;
        } else {
            playbackRegionStartMs = regionStartMs;
            playbackRegionEndMs = regionEndMs;
        }
        if (timer) {
            timer->setMode(mode, burstcon::msToBurstIndex(regionStartMs), burstcon::msToBurstIndex(regionEndMs));
        }
        if ((mode == burstcon::PlaybackMode::Region || mode == burstcon::PlaybackMode::RegionLoop) &&
            !running.load()) {
            const qint64 prefetchEnd = std::min(regionEndMs, regionStartMs + kPrefetchAheadMs);
            if (prefetchEnd > regionStartMs) {
                regionPrefetchStart = regionStartMs;
                regionPrefetchEnd = prefetchEnd;
                wantRegionIdlePrefetch = true;
            }
        }
    }
    if (clearRegionHeadBuckets) {
        std::lock_guard<std::mutex> cov(coverageMutex);
        timelineVideoRegionHeadBuckets.clear();
    }
    if (wantRegionIdlePrefetch &&
        !isPlaybackHeadWarm(regionPrefetchStart, regionPrefetchEnd - regionPrefetchStart)) {
        requestPrefetchAsync(regionPrefetchStart, regionPrefetchEnd, true);
    }
}

qint64 PlaybackPipeline::playbackPositionMs() const
{
    if (!running.load()) {
        return timelineMs.load();
    }
    return burstcon::burstIndexToMs(timer ? timer->currentIndex() : lastPlaybackIndex.load());
}

void PlaybackPipeline::setDraftPreviewEnabled(bool enabled)
{
    draftPreviewEnabled.store(enabled);
}

void PlaybackPipeline::setAudioLevelsCallback(AudioLevelsCallback cb)
{
    onAudioLevelsReady = std::move(cb);
}
#include <cmath>
std::pair<float, float> PlaybackPipeline::peaksDbFromAudioBurst(const burstcon::AudioBurst &burst, float gainLinear)
{
    constexpr float kSilenceDb = -60.f;
    if (burst.samples.empty()) {
        return {kSilenceDb, kSilenceDb};
    }
    const int ch = std::max(1, burst.channels);
    int peakL = 0;
    int peakR = 0;
    const float g = std::max(0.f, gainLinear);
    for (size_t s = 0; s + static_cast<size_t>(ch) <= burst.samples.size(); s += static_cast<size_t>(ch)) {
        const int v0 = static_cast<int>(
            std::llround(std::clamp(static_cast<double>(burst.samples[s]) * static_cast<double>(g),
                                    static_cast<double>(std::numeric_limits<int16_t>::min()),
                                    static_cast<double>(std::numeric_limits<int16_t>::max()))));
        peakL = std::max(peakL, std::abs(v0));
        if (ch >= 2) {
            const int v1 = static_cast<int>(
                std::llround(std::clamp(static_cast<double>(burst.samples[s + 1]) * static_cast<double>(g),
                                        static_cast<double>(std::numeric_limits<int16_t>::min()),
                                        static_cast<double>(std::numeric_limits<int16_t>::max()))));
            peakR = std::max(peakR, std::abs(v1));
        } else {
            peakR = peakL;
        }
    }
    auto toDb = [](int peak) {
        if (peak < 2) {
            return -60.f;
        }
        const float norm = static_cast<float>(peak) / 32768.f;
        return std::clamp(20.f * std::log10(norm), -60.f, 6.f);
    };
    return {toDb(peakL), toDb(peakR)};
}

bool PlaybackPipeline::fetchMixedAudioBurstAt(qint64 timelineMs, burstcon::AudioBurst &out) const
{
    std::shared_ptr<burstcon::IBurstcon> src;
    {
        std::lock_guard<std::mutex> lock(mutex);
        src = root;
    }
    if (!src) {
        return false;
    }
    constexpr int kRate = 48000;
    constexpr int kSpb = 1024;
    const burstcon::BurstIndex audioStep = std::max<burstcon::BurstIndex>(
        1, (burstcon::kBurstTimebaseHz * static_cast<burstcon::BurstIndex>(kSpb)) / kRate);
    const burstcon::BurstIndex rawIdx = burstcon::msToBurstIndex(timelineMs);
    const burstcon::BurstIndex idx = (rawIdx / audioStep) * audioStep;
    burstcon::BurstRequest req;
    req.index = idx;
    return src->audioBurst(req, out) && !out.samples.empty();
}

std::pair<float, float> PlaybackPipeline::sampleMixedLevelsDbAt(qint64 timelineMs) const
{
    burstcon::AudioBurst ab;
    if (!fetchMixedAudioBurstAt(timelineMs, ab)) {
        return {-60.f, -60.f};
    }
    return peaksDbFromAudioBurst(ab, previewVolumeLinear.load());
}

void PlaybackPipeline::ensureIdlePreviewAudio()
{
    ensureAudioOutput();
}

void PlaybackPipeline::beginScrubPlayback(qint64 timelineMs)
{
    const qint64 tMs = std::max<qint64>(0, timelineMs);
    scrubPlaybackActive.store(true);
    preparePlaybackAt(tMs);
    if (timer) {
        timer->stop();
        timer->emitAt(burstcon::msToBurstIndex(tMs));
        notifyAudioPcmReady();
    }
}

void PlaybackPipeline::scrubPlaybackSeek(qint64 positionMs)
{
    if (!scrubPlaybackActive.load()) {
        return;
    }
    const qint64 tMs = std::max<qint64>(0, positionMs);
    timelineMs.store(tMs);
    if (timer) {
        timer->emitAt(burstcon::msToBurstIndex(tMs));
        notifyAudioPcmReady();
    }
}

void PlaybackPipeline::endScrubPlayback()
{
    if (!scrubPlaybackActive.load()) {
        return;
    }
    scrubPlaybackActive.store(false);
    running.store(false);
    if (timer) {
        timer->stop();
    }
    shutdownAudioOutput();
    latestDraftSerial.fetch_add(1);
}

void PlaybackPipeline::requestMixedLevelsSampleAsync(qint64 timelineMs)
{
    const bool notify = [&]() {
        std::lock_guard<std::mutex> lock(draftMutex);
        pendingDraftJob.kind = DraftJob::Kind::MixedLevelsSample;
        pendingDraftJob.timelineMs = timelineMs;
        const bool wasPending = draftJobPending;
        draftJobPending = true;
        return !wasPending;
    }();
    if (notify) {
        draftCv.notify_one();
    }
}

void PlaybackPipeline::setPreviewVolume(float linearGain)
{
    const float v = std::max(0.f, linearGain);
    previewVolumeLinear.store(v);
    std::lock_guard<std::mutex> lock(audioMutex);
    if (audioSink) {
        const float hw = std::min(1.f, v);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
        audioSink->setVolume(hw);
#else
        audioSink->setVolume(hw);
#endif
    }
}

float PlaybackPipeline::previewVolume() const
{
    return previewVolumeLinear.load();
}

QImage PlaybackPipeline::renderDraftFrame(qint64 tMs, const std::vector<Clip> &videoClips)
{
    int fps = 30;
    QSize fallbackSize(640, 360);
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (!clipsEqual(videoClips, lastVideoClips)) {
            rebuildGraph(videoClips, lastAudioClips);
            lastVideoClips = videoClips;
        }
        fps = std::max(1, projectSettings.fps);
        const QSize r = projectSettings.cacheResolution();
        fallbackSize = QSize(std::max(320, r.width()), std::max(180, r.height()));
    }
    burstcon::BurstRequest req;
    req.index = burstcon::alignBurstIndexToOutputVideoFrame(fps, burstcon::msToBurstIndex(tMs));
    req.quality = decodeRealtimePipelineVideoQuality(projectSettings);
    burstcon::VideoBurst out;
    if (root && root->videoBurst(req, out) && !out.frame.isNull()) {
        return out.frame;
    }
    const QString activePath = activeVideoClipPathAtMs(tMs, videoClips);
    if (activePath.isEmpty()) {
        return makeBlackFrame(fallbackSize.width(), fallbackSize.height());
    }
    if (!QFileInfo::exists(activePath)) {
        return makeDraftFallbackImage(fallbackSize.width(),
                                      fallbackSize.height(),
                                      QStringLiteral("Media offline"),
                                      QFileInfo(activePath).fileName());
    }
    return makeBlackFrame(fallbackSize.width(), fallbackSize.height());
}

void PlaybackPipeline::setProjectSettings(const ProjectSettings &settings)
{
    std::lock_guard<std::mutex> lock(mutex);
    projectSettings = settings;
    if (timer) {
        timer->setVideoRequestQuality(decodeRealtimePipelineVideoQuality(projectSettings));
        timer->setPlaybackVideoFps(std::max(1, projectSettings.fps));
    }
    if (videoTrack) {
        videoTrack->setOutputVideoSize(decodeRealtimePipelineVideoQuality(projectSettings));
    }
    if (videoTrack && (!lastVideoClips.empty() || !lastAudioClips.empty())) {
        rebuildGraph(lastVideoClips, lastAudioClips);
    }
}

void PlaybackPipeline::resetProjectCache()
{
    std::lock_guard<std::mutex> lock(mutex);
    mediaPool->invalidatePath(QString());
    burstcon::resetMetrics();
    clearTimelineCoverage();
}

void PlaybackPipeline::clearTimelineCoverage()
{
    std::lock_guard<std::mutex> coverageLock(coverageMutex);
    audioCoverageBuckets.clear();
    timelineVideoSlidingBuckets.clear();
    timelineVideoRegionHeadBuckets.clear();
}

void PlaybackPipeline::clearVideoFrameBucketSets()
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    timelineVideoSlidingBuckets.clear();
    timelineVideoRegionHeadBuckets.clear();
}

void PlaybackPipeline::onVideoFrameCacheKeyChanged(bool inserted,
                                                   const QString &mediaPath,
                                                   burstcon::BurstIndex mediaBurstIndex,
                                                   const burstcon::BurstQuality &q)
{
    (void)q;
    std::vector<Clip> vclips;
    qint64 regStart = 0;
    qint64 regEnd = 0;
    {
        std::lock_guard<std::mutex> lock(mutex);
        vclips = lastVideoClips;
        regStart = playbackRegionStartMs;
        regEnd = playbackRegionEndMs;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : vclips) {
        if (clip.sourcePath != mediaPath) {
            continue;
        }
        const burstcon::BurstIndex eventStart = burstcon::msToBurstIndex(clip.startMs);
        const burstcon::BurstIndex eventDur = msToBurstIndexCeil(clip.durationMs);
        const burstcon::BurstIndex eventSourceIn = burstcon::msToBurstIndex(clip.sourceInMs);
        const burstcon::BurstIndex composition = eventStart + (mediaBurstIndex - eventSourceIn);
        if (composition < eventStart || composition >= eventStart + eventDur) {
            continue;
        }
        const qint64 tMs = burstcon::burstIndexToMs(composition);
        const qint64 bucket = videoCoverageBucketCenterMs(tMs);
        if (inserted) {
            timelineVideoSlidingBuckets.insert(bucket);
        } else {
            timelineVideoSlidingBuckets.erase(bucket);
        }
    }
}

void PlaybackPipeline::onAudioBurstCacheKeyChanged(bool inserted,
                                                   const QString &mediaPath,
                                                   burstcon::BurstIndex mediaBurstIndex)
{
    std::vector<Clip> aclips;
    {
        std::lock_guard<std::mutex> lock(mutex);
        aclips = lastAudioClips;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : aclips) {
        if (clip.sourcePath != mediaPath) {
            continue;
        }
        const burstcon::BurstIndex eventStart = burstcon::msToBurstIndex(clip.startMs);
        const burstcon::BurstIndex eventDur = msToBurstIndexCeil(clip.durationMs);
        const burstcon::BurstIndex eventSourceIn = burstcon::msToBurstIndex(clip.sourceInMs);
        const burstcon::BurstIndex composition = eventStart + (mediaBurstIndex - eventSourceIn);
        if (composition < eventStart || composition >= eventStart + eventDur) {
            continue;
        }
        const qint64 tMs = burstcon::burstIndexToMs(composition);
        const qint64 bucket = (tMs / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
        if (inserted) {
            audioCoverageBuckets.insert(bucket);
        } else {
            audioCoverageBuckets.erase(bucket);
        }
    }
}

PlaybackPipeline::PerformanceStats PlaybackPipeline::performanceSnapshot() const
{
    PerformanceStats stats;
    const auto m = burstcon::metricsSnapshot();
    stats.composeFps = composeFps.load();
    stats.composedFrames = composedFrames.load();
    stats.resourceCacheHits = m.cacheHits;
    stats.resourceCacheMisses = m.cacheMisses;
    stats.resourceCacheEntries = m.cacheEntries;
    stats.resourceCacheEvictions = m.cacheVideoUnusedEvictions;
    stats.avcppOpenCalls = m.avcppOpenCalls;
    stats.avcppPacketReads = m.avcppPacketReads;
    stats.avcppVideoDecodeCalls = m.avcppVideoDecodeCalls;
    stats.avcppAudioDecodeCalls = m.avcppAudioDecodeCalls;
    return stats;
}

QHash<QString, QVector<QPair<double, double>>> PlaybackPipeline::videoFrameSlidingSegments(const std::vector<Clip> &clips) const
{
    QHash<QString, QVector<QPair<double, double>>> out;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : clips) {
        const qint64 start = clip.startMs;
        const qint64 end = clip.startMs + std::max<qint64>(1, clip.durationMs);
        out.insert(coverageClipKey(clip), segmentsFromBucketsAlongSpan(timelineVideoSlidingBuckets, start, end));
    }
    return out;
}

QHash<QString, QVector<QPair<double, double>>> PlaybackPipeline::videoFrameHeadSegments(const std::vector<Clip> &clips) const
{
    qint64 regStart = 0;
    qint64 regEnd = 0;
    {
        std::lock_guard<std::mutex> g(mutex);
        regStart = playbackRegionStartMs;
        regEnd = playbackRegionEndMs;
    }
    QHash<QString, QVector<QPair<double, double>>> out;
    std::lock_guard<std::mutex> lock(coverageMutex);
    std::unordered_set<qint64> merged = timelineVideoSlidingBuckets;
    for (qint64 b : timelineVideoRegionHeadBuckets) {
        merged.insert(b);
    }
    for (const auto &clip : clips) {
        const qint64 dur = std::max<qint64>(1, clip.durationMs);
        out.insert(coverageClipKey(clip),
                   headSegmentsOnClip(merged, clip.startMs, dur, regStart, regEnd));
    }
    return out;
}

QVector<QPair<double, double>> PlaybackPipeline::timelineVideoSlidingSegments(qint64 viewStartMs,
                                                                              qint64 viewEndMs,
                                                                              qint64 forwardAnchorMs) const
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    return segmentsFromBucketsAlongSpanForward(timelineVideoSlidingBuckets,
                                               viewStartMs,
                                               viewEndMs,
                                               forwardAnchorMs);
}

QVector<QPair<double, double>> PlaybackPipeline::timelineVideoHeadSegments(qint64 viewStartMs, qint64 viewEndMs) const
{
    qint64 regStart = 0;
    qint64 regEnd = 0;
    {
        std::lock_guard<std::mutex> g(mutex);
        regStart = playbackRegionStartMs;
        regEnd = playbackRegionEndMs;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    std::unordered_set<qint64> merged = timelineVideoSlidingBuckets;
    for (qint64 b : timelineVideoRegionHeadBuckets) {
        merged.insert(b);
    }
    return headSegmentsOnView(merged, viewStartMs, viewEndMs, regStart, regEnd);
}

QVector<QPair<double, double>> PlaybackPipeline::timelineAudioSlidingSegments(qint64 viewStartMs,
                                                                            qint64 viewEndMs,
                                                                            qint64 forwardAnchorMs) const
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    return segmentsFromBucketsAlongSpanAudioForward(audioCoverageBuckets,
                                                    viewStartMs,
                                                    viewEndMs,
                                                    forwardAnchorMs);
}

QHash<QString, QVector<QPair<double, double>>> PlaybackPipeline::audioCacheSegments(const std::vector<Clip> &clips) const
{
    QHash<QString, QVector<QPair<double, double>>> out;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : clips) {
        QVector<QPair<double, double>> segs;
        const qint64 start = clip.startMs;
        const qint64 end = clip.startMs + std::max<qint64>(1, clip.durationMs);
        bool inRun = false;
        qint64 runStart = start;
        const qint64 stepMs = coverageTimeStepMs(end - start, kAudioCoverageBucketMs);
        for (qint64 t = start; t < end; t += stepMs) {
            const qint64 bucket = (t / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
            const bool hit = audioCoverageBuckets.find(bucket) != audioCoverageBuckets.end();
            if (hit && !inRun) {
                inRun = true;
                runStart = t;
            } else if (!hit && inRun) {
                const double a = static_cast<double>(runStart - start) / static_cast<double>(std::max<qint64>(1, end - start));
                const double b = static_cast<double>(t - start) / static_cast<double>(std::max<qint64>(1, end - start));
                segs.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
                inRun = false;
            }
        }
        if (inRun) {
            const double a = static_cast<double>(runStart - start) / static_cast<double>(std::max<qint64>(1, end - start));
            segs.push_back({std::clamp(a, 0.0, 1.0), 1.0});
        }
        out.insert(coverageClipKey(clip), segs);
    }
    return out;
}

QString PlaybackPipeline::coverageClipKey(const Clip &clip)
{
    // Must match TimelineEditor::clipCoverageKey() or cache stripes never paint on clips.
    return QStringLiteral("%1|%2|%3|%4")
        .arg(clip.sourcePath)
        .arg(clip.startMs)
        .arg(clip.durationMs)
        .arg(clip.sourceInMs);
}

bool PlaybackPipeline::clipsEqual(const std::vector<Clip> &a, const std::vector<Clip> &b)
{
    if (a.size() != b.size()) {
        return false;
    }
    for (size_t i = 0; i < a.size(); ++i) {
        if (a[i].sourcePath != b[i].sourcePath ||
            a[i].startMs != b[i].startMs ||
            a[i].durationMs != b[i].durationMs ||
            a[i].sourceInMs != b[i].sourceInMs ||
            a[i].trackIndex != b[i].trackIndex ||
            a[i].videoFxChain != b[i].videoFxChain ||
            a[i].audioFxChain != b[i].audioFxChain ||
            a[i].audioLinearGain != b[i].audioLinearGain) {
            return false;
        }
    }
    return true;
}

void PlaybackPipeline::rebuildGraph(const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips)
{
    const burstcon::BurstQuality outVideoQ = decodeRealtimePipelineVideoQuality(projectSettings);
    videoTrack->setOutputVideoSize(outVideoQ);

    std::vector<burstcon::ConTrackEvent> v;
    std::vector<burstcon::ConTrackEvent> a;
    for (const auto &clip : videoClips) {
        burstcon::ConTrackEvent e;
        e.sourcePath = clip.sourcePath;
        e.start = burstcon::msToBurstIndex(clip.startMs);
        e.duration = msToBurstIndexCeil(clip.durationMs);
        e.sourceIn = burstcon::msToBurstIndex(clip.sourceInMs);
        e.hasVideo = true;
        e.hasAudio = false;
        e.timelineLaneIndex = clip.trackIndex;
        e.videoFxChain = clip.videoFxChain;
        // Ascending z = background first: lower track index (upper row in UI) composites on top.
        const qint64 startKey = std::min<qint64>(std::max<qint64>(0, clip.startMs), 999'999LL);
        e.zIndex = -static_cast<int>(clip.trackIndex * 1'000'000LL + startKey);
        v.push_back(e);
    }
    int z = 0;
    for (const auto &clip : audioClips) {
        burstcon::ConTrackEvent e;
        e.sourcePath = clip.sourcePath;
        e.start = burstcon::msToBurstIndex(clip.startMs);
        e.duration = msToBurstIndexCeil(clip.durationMs);
        e.sourceIn = burstcon::msToBurstIndex(clip.sourceInMs);
        e.hasVideo = false;
        e.hasAudio = true;
        e.timelineLaneIndex = clip.trackIndex;
        e.audioFxChain = clip.audioFxChain;
        e.audioLinearGain = clip.audioLinearGain;
        e.zIndex = z++;
        a.push_back(e);
    }
    videoTrack->setEvents(v);
    audioTrack->setEvents(a);
    trimmer->setTracks({videoTrack, audioTrack});
    root = trimmer;
    if (timer) {
        timer->setSource(root);
    }
    clearVideoFrameBucketSets();
    // Invalidate in-flight draft jobs so idle preview does not keep showing frames from the old FX graph.
    latestDraftSerial.fetch_add(1);
}

void PlaybackPipeline::tearDownAudioLocked()
{
    if (audioSink) {
        audioSink->stop();
        delete audioSink;
        audioSink = nullptr;
    }
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    delete pcmFeedDevice;
    pcmFeedDevice = nullptr;
#endif
    audioDevice = nullptr;
    audioPcmPending.clear();
}

void PlaybackPipeline::ensureAudioOutput()
{
    std::lock_guard<std::mutex> lock(audioMutex);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    if (audioSink && pcmFeedDevice) {
        return;
    }
#else
    if (audioSink && audioDevice) {
        return;
    }
#endif
    tearDownAudioLocked();

    QAudioFormat format;
    format.setSampleRate(kAudioSampleRate);
    format.setChannelCount(kAudioChannels);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    format.setSampleFormat(QAudioFormat::Int16);
    const QAudioDevice output = QMediaDevices::defaultAudioOutput();
    if (!output.isFormatSupported(format)) {
        bool foundInt16 = false;
        for (const QAudioFormat &candidate : output.supportedFormats()) {
            if (candidate.sampleFormat() == QAudioFormat::Int16 &&
                candidate.channelCount() == kAudioChannels &&
                candidate.sampleRate() == kAudioSampleRate) {
                format = candidate;
                foundInt16 = true;
                break;
            }
        }
        if (!foundInt16) {
            format = output.preferredFormat();
        }
    }
    audioOutputFormat = format;
    pcmFeedDevice = new PreviewPcmIODevice(this);
    audioSink = new QAudioSink(output, format);
    audioSink->setBufferSize(64 * 1024);
    audioSink->start(pcmFeedDevice);
    audioDevice = nullptr;
    const float hw = std::min(1.f, previewVolumeLinear.load());
    audioSink->setVolume(hw);
    if (audioSink->state() == QAudio::SuspendedState || audioSink->state() == QAudio::IdleState) {
        audioSink->resume();
    }
#else
    format.setSampleSize(16);
    format.setCodec("audio/pcm");
    format.setByteOrder(QAudioFormat::LittleEndian);
    format.setSampleType(QAudioFormat::SignedInt);
    audioSink = new QAudioOutput(format);
    audioDevice = audioSink->start();
    if (audioDevice) {
        const float hw = std::min(1.f, previewVolumeLinear.load());
        audioSink->setVolume(hw);
    } else {
        tearDownAudioLocked();
    }
#endif
}

void PlaybackPipeline::shutdownAudioOutput()
{
    std::lock_guard<std::mutex> lock(audioMutex);
    tearDownAudioLocked();
}

void PlaybackPipeline::flushAudioPendingToDevice()
{
    if (!audioDevice || audioPcmPending.empty()) {
        return;
    }
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    if (audioSink) {
        const QAudio::State st = audioSink->state();
        if (st == QAudio::SuspendedState) {
            audioSink->resume();
        } else if (st == QAudio::IdleState) {
            audioSink->resume();
        }
    }
#endif
    constexpr int kMaxWriteIters = 96;
    int iters = 0;
    while (!audioPcmPending.empty() && iters < kMaxWriteIters) {
        const char *bytes = reinterpret_cast<const char *>(audioPcmPending.data());
        int len = static_cast<int>(audioPcmPending.size() * sizeof(int16_t));
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
        if (audioSink) {
            const qsizetype freeBytes = audioSink->bytesFree();
            if (freeBytes > 0 && freeBytes < static_cast<qsizetype>(len)) {
                len = static_cast<int>(freeBytes);
                len -= len % static_cast<int>(sizeof(int16_t));
            }
        }
#endif
        if (len <= 0) {
            break;
        }
        const qint64 w = audioDevice->write(bytes, len);
        if (w < 0) {
            audioPcmPending.clear();
            return;
        }
        if (w == 0) {
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
            if (audioSink && audioSink->state() == QAudio::IdleState) {
                audioSink->resume();
            }
#endif
            break;
        }
        const int consumed = static_cast<int>(w) / static_cast<int>(sizeof(int16_t));
        if (consumed <= 0) {
            break;
        }
        audioPcmPending.erase(audioPcmPending.begin(), audioPcmPending.begin() + consumed);
        ++iters;
    }
}

void PlaybackPipeline::pushAudioBurst(const burstcon::AudioBurst &burst)
{
    // QAudioOutput/QAudioSink must be created and destroyed on the same thread (Qt object thread
    // affinity). This callback runs on `MonotonicPlaybackTimer`'s thread — never (re)open audio here,
    // or `stop()` on the GUI thread will delete a foreign-thread QObject and corrupt the heap.
    if (burst.samples.empty()) {
        return;
    }
    const float gain = previewVolumeLinear.load();
    const auto levels = peaksDbFromAudioBurst(burst, gain);
    if (onAudioLevelsReady) {
        onAudioLevelsReady(levels.first, levels.second);
    }

    {
        std::lock_guard<std::mutex> lock(audioMutex);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
        if (!audioSink || !pcmFeedDevice) {
            return;
        }
#else
        if (!audioDevice) {
            return;
        }
#endif
        constexpr size_t kMaxPendingStereoSamples = static_cast<size_t>(48000) * 2 * 8;
        const size_t prev = audioPcmPending.size();
        audioPcmPending.resize(prev + burst.samples.size());
        for (size_t i = 0; i < burst.samples.size(); ++i) {
            const int32_t scaled =
                static_cast<int32_t>(std::llround(static_cast<double>(burst.samples[i]) * static_cast<double>(gain)));
            const int32_t v = std::clamp<int32_t>(scaled, -32768, 32767);
            audioPcmPending[prev + i] = static_cast<int16_t>(v);
        }
        if (audioPcmPending.size() > kMaxPendingStereoSamples) {
            const size_t drop = audioPcmPending.size() - kMaxPendingStereoSamples;
            audioPcmPending.erase(audioPcmPending.begin(),
                                  audioPcmPending.begin() + static_cast<std::ptrdiff_t>(drop));
        }
    }
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    notifyAudioPcmReady();
#else
    flushAudioPendingToDevice();
#endif
}

void PlaybackPipeline::clearAudioPendingLocked()
{
    audioPcmPending.clear();
}

qint64 PlaybackPipeline::readAudioPcmForDevice(char *data, qint64 maxBytes)
{
    if (!data || maxBytes <= 0) {
        return 0;
    }
    std::lock_guard<std::mutex> lock(audioMutex);
    const qint64 available =
        static_cast<qint64>(audioPcmPending.size()) * static_cast<qint64>(sizeof(int16_t));
    const qint64 toRead = std::min(maxBytes, available);
    if (toRead <= 0) {
        return 0;
    }
    std::memcpy(data, audioPcmPending.data(), static_cast<size_t>(toRead));
    const size_t samplesConsumed = static_cast<size_t>(toRead / static_cast<qint64>(sizeof(int16_t)));
    audioPcmPending.erase(audioPcmPending.begin(),
                            audioPcmPending.begin() + static_cast<std::ptrdiff_t>(samplesConsumed));
    return toRead;
}

void PlaybackPipeline::notifyAudioPcmReady()
{
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    if (pcmFeedDevice) {
        (void)QMetaObject::invokeMethod(pcmFeedDevice, "readyRead", Qt::QueuedConnection);
    }
#endif
}

void PlaybackPipeline::requestIdlePreviewAsync(qint64 tMs, const std::vector<Clip> &videoClips)
{
    const bool notify = [&]() {
        std::lock_guard<std::mutex> lock(draftMutex);
        pendingDraftJob.kind = DraftJob::Kind::IdlePreview;
        pendingDraftJob.timelineMs = tMs;
        pendingDraftJob.videoClips = videoClips;
        const bool wasPending = draftJobPending;
        draftJobPending = true;
        return !wasPending;
    }();
    if (notify) {
        draftCv.notify_one();
    }
}

void PlaybackPipeline::draftWorkerLoop()
{
    while (true) {
        DraftJob job;
        bool haveDraft = false;
        {
            std::unique_lock<std::mutex> lock(draftMutex);
            draftCv.wait(lock, [this]() { return stopDraftWorker || draftJobPending; });
            if (stopDraftWorker) {
                return;
            }
            if (draftJobPending) {
                job = pendingDraftJob;
                draftJobPending = false;
                haveDraft = true;
            }
        }
        if (!haveDraft) {
            continue;
        }
        if (job.kind == DraftJob::Kind::MixedLevelsSample) {
            const auto levels = sampleMixedLevelsDbAt(job.timelineMs);
            if (onAudioLevelsReady) {
                onAudioLevelsReady(levels.first, levels.second);
            }
            continue;
        }
        if (running.load() || !draftPreviewEnabled.load()) {
            continue;
        }
        for (;;) {
            DraftJob previewJob = job;
            {
                std::lock_guard<std::mutex> lock(draftMutex);
                if (draftJobPending && pendingDraftJob.kind == DraftJob::Kind::IdlePreview) {
                    previewJob = pendingDraftJob;
                    draftJobPending = false;
                }
            }
            QImage frame = renderDraftFrame(previewJob.timelineMs, previewJob.videoClips);
            if (running.load()) {
                break;
            }
            bool superseded = false;
            {
                std::lock_guard<std::mutex> lock(draftMutex);
                if (draftJobPending && pendingDraftJob.kind == DraftJob::Kind::IdlePreview) {
                    superseded = true;
                }
            }
            if (superseded) {
                continue;
            }
            if (onVideoFrameReady) {
                onVideoFrameReady(std::move(frame));
            }
            break;
        }
    }
}

void PlaybackPipeline::requestPrefetchAsync(qint64 startMs, qint64 endMs, bool allowWhileRunning)
{
    if (endMs <= startMs) {
        return;
    }
    PrefetchJob job;
    job.startMs = std::max<qint64>(0, startMs);
    job.endMs = std::max(job.startMs, endMs);
    job.allowWhileRunning = allowWhileRunning;
    job.serial = latestPrefetchSerial.fetch_add(1) + 1;
    lastPrefetchAnchorMs.store(job.startMs);
    {
        std::lock_guard<std::mutex> lock(prefetchMutex);
        pendingPrefetchJob = job;
        prefetchJobPending = true;
    }
    prefetchCv.notify_one();
}

void PlaybackPipeline::prefetchWorkerLoop()
{
    while (true) {
        PrefetchJob job;
        std::shared_ptr<burstcon::IBurstcon> localRoot;
        burstcon::BurstQuality quality;
        int fps = 30;
        {
            std::unique_lock<std::mutex> lock(prefetchMutex);
            prefetchCv.wait(lock, [this]() { return stopPrefetchWorker || prefetchJobPending; });
            if (stopPrefetchWorker) {
                return;
            }
            job = pendingPrefetchJob;
            prefetchJobPending = false;
        }
        burstcon::PlaybackMode pbMode = burstcon::PlaybackMode::Free;
        qint64 regStartMs = 0;
        qint64 regEndMs = 0;
        {
            std::lock_guard<std::mutex> lock(mutex);
            localRoot = root;
            quality = decodeRealtimePipelineVideoQuality(projectSettings);
            fps = std::max(1, projectSettings.fps);
            pbMode = playbackMode;
            regStartMs = playbackRegionStartMs;
            regEndMs = playbackRegionEndMs;
        }
        if (!localRoot) {
            continue;
        }
        if (!job.allowWhileRunning && running.load()) {
            continue;
        }
        qint64 runEndMs = job.endMs;
        if (running.load() && job.allowWhileRunning) {
            runEndMs = std::min(runEndMs, job.startMs + kPrefetchDuringPlayAheadMs);
        }
        if (runEndMs <= job.startMs) {
            continue;
        }
        if (isPlaybackHeadWarm(job.startMs, runEndMs - job.startMs)) {
            continue;
        }
        const burstcon::BurstIndex videoStep = burstcon::outputVideoFrameStepTicks(fps);
        const burstcon::BurstIndex audioStep = std::max<burstcon::BurstIndex>(
            1, (burstcon::kBurstTimebaseHz * static_cast<burstcon::BurstIndex>(kDefaultAudioSamplesPerBurst)) / kAudioSampleRate);
        const burstcon::BurstIndex startIdx = burstcon::msToBurstIndex(job.startMs);
        const burstcon::BurstIndex alignedStart = burstcon::alignBurstIndexToOutputVideoFrame(fps, startIdx);
        const burstcon::BurstIndex endIdx = msToBurstIndexCeil(runEndMs);
        const qint64 prefetchSpanMs = runEndMs - job.startMs;
        const burstcon::BurstIndex prefetchVideoStep =
            prefetchSpanMs <= kPrefetchVideoDenseMaxSpanMs
                ? videoStep
                : std::max(videoStep, burstcon::kBurstTimebaseHz / 12);
        for (burstcon::BurstIndex idx = alignedStart; idx <= endIdx; idx += prefetchVideoStep) {
            if (job.serial != latestPrefetchSerial.load()) {
                break;
            }
            burstcon::BurstRequest req;
            req.index = idx;
            req.quality = quality;
            burstcon::VideoBurst vb;
            if (localRoot->videoBurst(req, vb) && !vb.frame.isNull()) {
                const qint64 tMs = burstcon::burstIndexToMs(idx);
                if ((pbMode == burstcon::PlaybackMode::Region || pbMode == burstcon::PlaybackMode::RegionLoop) &&
                    regEndMs > regStartMs) {
                    const qint64 headEnd = std::min(regEndMs, regStartMs + kRegionHeadWindowMs);
                    if (tMs >= regStartMs && tMs < headEnd) {
                        std::lock_guard<std::mutex> cov(coverageMutex);
                        timelineVideoRegionHeadBuckets.insert(videoCoverageBucketCenterMs(tMs));
                    }
                }
            }
        }
        const burstcon::BurstIndex alignedAudioStart = (startIdx / audioStep) * audioStep;
        for (burstcon::BurstIndex idx = alignedAudioStart; idx <= endIdx; idx += audioStep) {
            if (job.serial != latestPrefetchSerial.load()) {
                break;
            }
            burstcon::BurstRequest req;
            req.index = idx;
            burstcon::AudioBurst ab;
            if (localRoot->audioBurst(req, ab)) {
                std::lock_guard<std::mutex> lock(coverageMutex);
                const qint64 tMs = burstcon::burstIndexToMs(idx);
                const qint64 bucket = (tMs / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
                audioCoverageBuckets.insert(bucket);
            }
        }
    }
}
