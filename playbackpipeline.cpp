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
#include <QIODevice>

#include <algorithm>
#include <chrono>
#include <thread>
#include <vector>

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr int kDefaultAudioSamplesPerBurst = 1024;
constexpr qint64 kVideoCoverageBucketMs = 33;
constexpr qint64 kAudioCoverageBucketMs = 20;
constexpr qint64 kPrefetchAheadMs = 12000;
/// When scrubbing while stopped, only look this far ahead so `isPlaybackHeadWarm` matches the usual
/// 5 s warmup window from MainWindow / `start()` and idle skips actually trigger.
constexpr qint64 kIdleScrubPrefetchAheadMs = 5000;
constexpr qint64 kPrefetchPlayRescheduleMs = 450;
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
        const qint64 bucket = (t / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
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

} // namespace

constexpr qint64 kRegionHeadVideoCacheMs = 1500;

PlaybackPipeline::PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady)
    : onVideoFrameReady(std::move(onVideoFrameReady))
{
    burstcon::ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = true;
    poolCfg.cacheConfig.maxEntries = 256;
    poolCfg.cacheConfig.maxAudioEntries = 128;
    poolCfg.cacheConfig.maxVideoEntries = 192;
    poolCfg.cacheConfig.maxVideoBytes = 384ULL * 1024ULL * 1024ULL;
    poolCfg.cacheConfig.preloadEnabled = false;
    poolCfg.cacheConfig.preloadAheadTicks = burstcon::kBurstTimebaseHz;
    poolCfg.maxMediaInstances = 16;
    poolCfg.cacheConfig.onVideoFrameCacheChanged =
        [this](bool inserted, const QString &mediaPath, burstcon::BurstIndex mediaBurstIndex, const burstcon::BurstQuality &q) {
            onVideoFrameCacheKeyChanged(inserted, mediaPath, mediaBurstIndex, q);
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
        if (onVideoFrameReady && !burst.frame.isNull()) {
            onVideoFrameReady(std::move(burst.frame));
            composedFrames.fetch_add(1);
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
    const QSize q = projectSettings.cacheResolution();
    {
        std::lock_guard<std::mutex> lock(mutex);
        timer->setVideoRequestQuality(burstcon::BurstQuality{q.width(), q.height()});
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
    {
        std::lock_guard<std::mutex> lock(mutex);
        localRoot = root;
        const QSize q = projectSettings.cacheResolution();
        quality = burstcon::BurstQuality{q.width(), q.height()};
        fps = std::max(1, projectSettings.fps);
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
        (void)localRoot->videoBurst(req, vb);
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
    qint64 prefetchEndMs = startTimelineMs + kPrefetchAheadMs;
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
    bool projectHasAudio = false;
    {
        std::lock_guard<std::mutex> lock(mutex);
        if ((playbackMode == burstcon::PlaybackMode::Region || playbackMode == burstcon::PlaybackMode::RegionLoop) &&
            playbackRegionEndMs > playbackRegionStartMs) {
            endMs = std::min(endMs, playbackRegionEndMs);
        }
        projectHasAudio = !lastAudioClips.empty();
    }
    if (endMs <= startMs) {
        return true;
    }
    constexpr qint64 kProbeStepMs = 400;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (qint64 t = startMs; t < endMs; t += kProbeStepMs) {
        const qint64 bucket = (t / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
        if (timelineVideoSlidingBuckets.find(bucket) == timelineVideoSlidingBuckets.end()) {
            return false;
        }
        if (projectHasAudio) {
            const qint64 aBucket = (t / kAudioCoverageBucketMs) * kAudioCoverageBucketMs;
            if (audioCoverageBuckets.find(aBucket) == audioCoverageBuckets.end()) {
                return false;
            }
        }
    }
    const qint64 lastT = std::max(startMs, endMs - 1);
    const qint64 bLast = (lastT / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
    return timelineVideoSlidingBuckets.find(bLast) != timelineVideoSlidingBuckets.end();
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
        if (onVideoFrameReady && !headWarm) {
            requestDraftFrameAsync(tMs, videoClips);
        }
        if (idleSpan > 0 && !headWarm) {
            requestPrefetchAsync(tMs, idlePrefetchEnd, true);
        }
    } else {
        const qint64 anchor = lastPrefetchAnchorMs.load();
        if (tMs > anchor + kPrefetchPlayRescheduleMs) {
            lastPrefetchAnchorMs.store(tMs);
            const qint64 span = prefetchEndMs - tMs;
            if (span > 0 && !isPlaybackHeadWarm(tMs, span)) {
                requestPrefetchAsync(tMs, prefetchEndMs, true);
            }
        }
    }
}

void PlaybackPipeline::setPlaybackMode(burstcon::PlaybackMode mode, qint64 regionStartMs, qint64 regionEndMs)
{
    qint64 regionPrefetchStart = 0;
    qint64 regionPrefetchEnd = 0;
    bool wantRegionIdlePrefetch = false;
    {
        std::lock_guard<std::mutex> lock(mutex);
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

QImage PlaybackPipeline::renderDraftFrame(qint64 tMs, const std::vector<Clip> &videoClips)
{
    int fps = 30;
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (!clipsEqual(videoClips, lastVideoClips)) {
            rebuildGraph(videoClips, lastAudioClips);
            lastVideoClips = videoClips;
        }
        fps = std::max(1, projectSettings.fps);
    }
    burstcon::BurstRequest req;
    req.index = burstcon::alignBurstIndexToOutputVideoFrame(fps, burstcon::msToBurstIndex(tMs));
    req.quality = burstcon::BurstQuality{projectSettings.cacheResolution().width(), projectSettings.cacheResolution().height()};
    burstcon::VideoBurst out;
    if (root && root->videoBurst(req, out)) {
        return out.frame;
    }
    return QImage();
}

void PlaybackPipeline::setProjectSettings(const ProjectSettings &settings)
{
    std::lock_guard<std::mutex> lock(mutex);
    projectSettings = settings;
    if (timer) {
        const QSize q = projectSettings.cacheResolution();
        timer->setVideoRequestQuality(burstcon::BurstQuality{q.width(), q.height()});
        timer->setPlaybackVideoFps(std::max(1, projectSettings.fps));
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
    timelineVideoHeadBuckets.clear();
}

void PlaybackPipeline::clearVideoFrameBucketSets()
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    timelineVideoSlidingBuckets.clear();
    timelineVideoHeadBuckets.clear();
}

void PlaybackPipeline::onVideoFrameCacheKeyChanged(bool inserted,
                                                   const QString &mediaPath,
                                                   burstcon::BurstIndex mediaBurstIndex,
                                                   const burstcon::BurstQuality &q)
{
    QSize wantRes;
    std::vector<Clip> vclips;
    qint64 regStart = 0;
    qint64 regEnd = 0;
    {
        std::lock_guard<std::mutex> lock(mutex);
        wantRes = projectSettings.cacheResolution();
        vclips = lastVideoClips;
        regStart = playbackRegionStartMs;
        regEnd = playbackRegionEndMs;
    }
    if (q.width != wantRes.width() || q.height != wantRes.height()) {
        return;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    const qint64 headEnd = (regEnd > regStart) ? std::min(regEnd, regStart + kRegionHeadVideoCacheMs) : regEnd;
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
        const qint64 bucket = (tMs / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
        if (inserted) {
            timelineVideoSlidingBuckets.insert(bucket);
            if (regEnd > regStart && tMs >= regStart && tMs < headEnd) {
                timelineVideoHeadBuckets.insert(bucket);
            }
        } else {
            timelineVideoSlidingBuckets.erase(bucket);
            timelineVideoHeadBuckets.erase(bucket);
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
    QHash<QString, QVector<QPair<double, double>>> out;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : clips) {
        const qint64 start = clip.startMs;
        const qint64 end = clip.startMs + std::max<qint64>(1, clip.durationMs);
        out.insert(coverageClipKey(clip), segmentsFromBucketsAlongSpan(timelineVideoHeadBuckets, start, end));
    }
    return out;
}

QVector<QPair<double, double>> PlaybackPipeline::timelineVideoSlidingSegments(qint64 viewStartMs, qint64 viewEndMs) const
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    return segmentsFromBucketsAlongSpan(timelineVideoSlidingBuckets, viewStartMs, viewEndMs);
}

QVector<QPair<double, double>> PlaybackPipeline::timelineVideoHeadSegments(qint64 viewStartMs, qint64 viewEndMs) const
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    return segmentsFromBucketsAlongSpan(timelineVideoHeadBuckets, viewStartMs, viewEndMs);
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
            a[i].sourceInMs != b[i].sourceInMs) {
            return false;
        }
    }
    return true;
}

void PlaybackPipeline::rebuildGraph(const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips)
{
    std::vector<burstcon::ConTrackEvent> v;
    std::vector<burstcon::ConTrackEvent> a;
    int z = 0;
    for (const auto &clip : videoClips) {
        burstcon::ConTrackEvent e;
        e.sourcePath = clip.sourcePath;
        e.start = burstcon::msToBurstIndex(clip.startMs);
        e.duration = msToBurstIndexCeil(clip.durationMs);
        e.sourceIn = burstcon::msToBurstIndex(clip.sourceInMs);
        e.hasVideo = true;
        e.hasAudio = false;
        e.zIndex = z++;
        v.push_back(e);
    }
    z = 0;
    for (const auto &clip : audioClips) {
        burstcon::ConTrackEvent e;
        e.sourcePath = clip.sourcePath;
        e.start = burstcon::msToBurstIndex(clip.startMs);
        e.duration = msToBurstIndexCeil(clip.durationMs);
        e.sourceIn = burstcon::msToBurstIndex(clip.sourceInMs);
        e.hasVideo = false;
        e.hasAudio = true;
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
}

void PlaybackPipeline::tearDownAudioLocked()
{
    if (audioSink) {
        audioSink->stop();
        delete audioSink;
        audioSink = nullptr;
    }
    audioDevice = nullptr;
}

void PlaybackPipeline::ensureAudioOutput()
{
    std::lock_guard<std::mutex> lock(audioMutex);
    if (audioSink && audioDevice) {
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
        if (audioSink->state() != QAudio::StoppedState) {
            return;
        }
#else
        if (audioSink->state() != QAudio::StoppedState) {
            return;
        }
#endif
    }
    tearDownAudioLocked();

    QAudioFormat format;
    format.setSampleRate(kAudioSampleRate);
    format.setChannelCount(kAudioChannels);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    format.setSampleFormat(QAudioFormat::Int16);
    const QAudioDevice output = QMediaDevices::defaultAudioOutput();
    if (!output.isFormatSupported(format)) {
        format = output.preferredFormat();
    }
    audioSink = new QAudioSink(output, format);
#else
    format.setSampleSize(16);
    format.setCodec("audio/pcm");
    format.setByteOrder(QAudioFormat::LittleEndian);
    format.setSampleType(QAudioFormat::SignedInt);
    audioSink = new QAudioOutput(format);
#endif
    audioDevice = audioSink->start();
}

void PlaybackPipeline::shutdownAudioOutput()
{
    std::lock_guard<std::mutex> lock(audioMutex);
    tearDownAudioLocked();
}

void PlaybackPipeline::pushAudioBurst(const burstcon::AudioBurst &burst)
{
    // QAudioOutput/QAudioSink must be created and destroyed on the same thread (Qt object thread
    // affinity). This callback runs on `MonotonicPlaybackTimer`'s thread — never (re)open audio here,
    // or `stop()` on the GUI thread will delete a foreign-thread QObject and corrupt the heap.
    if (!audioDevice || burst.samples.empty()) {
        return;
    }
    std::vector<int16_t> pcm;
    pcm.resize(burst.samples.size(), 0);
    for (size_t i = 0; i < burst.samples.size(); ++i) {
        const int32_t v = std::clamp<int32_t>(burst.samples[i], -32768, 32767);
        pcm[i] = static_cast<int16_t>(v);
    }
    const char *bytes = reinterpret_cast<const char *>(pcm.data());
    const int len = static_cast<int>(pcm.size() * sizeof(int16_t));
    int written = 0;
    while (written < len && running.load()) {
        const qint64 w = audioDevice->write(bytes + written, len - written);
        if (w < 0) {
            break;
        }
        if (w == 0) {
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
            continue;
        }
        written += static_cast<int>(w);
    }
}

void PlaybackPipeline::requestDraftFrameAsync(qint64 tMs, const std::vector<Clip> &videoClips)
{
    DraftJob job;
    job.timelineMs = tMs;
    job.videoClips = videoClips;
    job.serial = latestDraftSerial.fetch_add(1) + 1;
    {
        std::lock_guard<std::mutex> lock(draftMutex);
        pendingDraftJob = std::move(job);
        draftJobPending = true;
    }
    draftCv.notify_one();
}

void PlaybackPipeline::draftWorkerLoop()
{
    while (true) {
        DraftJob job;
        {
            std::unique_lock<std::mutex> lock(draftMutex);
            draftCv.wait(lock, [this]() { return stopDraftWorker || draftJobPending; });
            if (stopDraftWorker) {
                return;
            }
            job = pendingDraftJob;
            draftJobPending = false;
        }
        if (running.load() || !draftPreviewEnabled.load()) {
            continue;
        }
        if (job.serial != latestDraftSerial.load()) {
            continue;
        }
        QImage frame = renderDraftFrame(job.timelineMs, job.videoClips);
        if (frame.isNull() || running.load()) {
            continue;
        }
        if (job.serial != latestDraftSerial.load()) {
            continue;
        }
        if (onVideoFrameReady) {
            onVideoFrameReady(std::move(frame));
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
        {
            std::lock_guard<std::mutex> lock(mutex);
            localRoot = root;
            const QSize q = projectSettings.cacheResolution();
            quality = burstcon::BurstQuality{q.width(), q.height()};
            fps = std::max(1, projectSettings.fps);
        }
        if (!localRoot) {
            continue;
        }
        if (!job.allowWhileRunning && running.load()) {
            continue;
        }
        if (job.endMs > job.startMs && isPlaybackHeadWarm(job.startMs, job.endMs - job.startMs)) {
            continue;
        }
        const burstcon::BurstIndex videoStep = burstcon::outputVideoFrameStepTicks(fps);
        const burstcon::BurstIndex audioStep = std::max<burstcon::BurstIndex>(
            1, (burstcon::kBurstTimebaseHz * static_cast<burstcon::BurstIndex>(kDefaultAudioSamplesPerBurst)) / kAudioSampleRate);
        const burstcon::BurstIndex startIdx = burstcon::msToBurstIndex(job.startMs);
        const burstcon::BurstIndex alignedStart = burstcon::alignBurstIndexToOutputVideoFrame(fps, startIdx);
        const burstcon::BurstIndex endIdx = msToBurstIndexCeil(job.endMs);
        const qint64 prefetchSpanMs = job.endMs - job.startMs;
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
            (void)localRoot->videoBurst(req, vb);
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
