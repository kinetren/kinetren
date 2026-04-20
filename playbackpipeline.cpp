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

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr qint64 kVideoCoverageBucketMs = 33;
constexpr qint64 kAudioCoverageBucketMs = 20;

burstcon::BurstIndex msToBurstIndexCeil(qint64 ms)
{
    const qint64 safe = std::max<qint64>(1, ms);
    return static_cast<burstcon::BurstIndex>((safe * burstcon::kBurstTimebaseHz + 999) / 1000);
}
}

PlaybackPipeline::PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady)
    : onVideoFrameReady(std::move(onVideoFrameReady))
{
    burstcon::ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = true;
    poolCfg.cacheConfig.maxEntries = 192;
    poolCfg.cacheConfig.maxAudioEntries = 96;
    poolCfg.cacheConfig.maxVideoEntries = 96;
    poolCfg.cacheConfig.maxVideoBytes = 192ULL * 1024ULL * 1024ULL;
    poolCfg.cacheConfig.preloadEnabled = true;
    poolCfg.cacheConfig.preloadAheadTicks = burstcon::kBurstTimebaseHz;
    poolCfg.maxMediaInstances = 16;
    mediaPool = std::make_shared<burstcon::ConMediaPool>(poolCfg);
    videoTrack = std::make_shared<burstcon::ConTrack>(mediaPool);
    audioTrack = std::make_shared<burstcon::ConTrack>(mediaPool);
    trimmer = std::make_shared<burstcon::ConTrimmer>();
    trimmer->setTracks({videoTrack, audioTrack});
    root = trimmer;
    timer = std::make_unique<burstcon::MonotonicPlaybackTimer>(root);
    draftWorker = std::thread([this]() { draftWorkerLoop(); });
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
}

void PlaybackPipeline::start(qint64 startTimelineMs)
{
    std::lock_guard<std::mutex> lock(mutex);
    running.store(true);
    latestDraftSerial.fetch_add(1);
    timelineMs.store(startTimelineMs);
    ensureAudioOutput();
    burstcon::MonotonicPlaybackTimer::Callbacks callbacks;
    callbacks.onVideo = [this](const burstcon::VideoBurst &burst) {
        const qint64 tMs = burstcon::burstIndexToMs(burst.index);
        timelineMs.store(tMs);
        lastPlaybackIndex.store(burst.index);
        {
            std::lock_guard<std::mutex> lock(coverageMutex);
            videoCoverageBuckets.insert((tMs / kVideoCoverageBucketMs) * kVideoCoverageBucketMs);
            finalCoverageBuckets.insert((tMs / kVideoCoverageBucketMs) * kVideoCoverageBucketMs);
        }
        if (onVideoFrameReady && !burst.frame.isNull()) {
            onVideoFrameReady(QImage(burst.frame));
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
    timer->setVideoRequestQuality(burstcon::BurstQuality{q.width(), q.height()});
    timer->setCallbacks(callbacks);
    lastVideoRequestIndex.store(-1);
    lastAudioRequestIndex.store(-1);
    timer->start(burstcon::msToBurstIndex(startTimelineMs));
}

void PlaybackPipeline::stop()
{
    std::lock_guard<std::mutex> lock(mutex);
    running.store(false);
    latestDraftSerial.fetch_add(1);
    if (timer) {
        timer->stop();
    }
    shutdownAudioOutput();
    lastVideoRequestIndex.store(-1);
    lastAudioRequestIndex.store(-1);
}

bool PlaybackPipeline::isRunning() const
{
    return timer && timer->isRunning();
}

void PlaybackPipeline::update(qint64 tMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips)
{
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (!clipsEqual(videoClips, lastVideoClips) || !clipsEqual(audioClips, lastAudioClips)) {
            rebuildGraph(videoClips, audioClips);
            lastVideoClips = videoClips;
            lastAudioClips = audioClips;
        }
    }
    if (!running.load()) {
        timelineMs.store(tMs);
        if (onVideoFrameReady) {
            requestDraftFrameAsync(tMs, videoClips);
        }
    }
}

void PlaybackPipeline::setPlaybackMode(burstcon::PlaybackMode mode, qint64 regionStartMs, qint64 regionEndMs)
{
    std::lock_guard<std::mutex> lock(mutex);
    if (timer) {
        timer->setMode(mode, burstcon::msToBurstIndex(regionStartMs), burstcon::msToBurstIndex(regionEndMs));
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
    {
        std::lock_guard<std::mutex> lock(mutex);
        if (!clipsEqual(videoClips, lastVideoClips)) {
            rebuildGraph(videoClips, lastAudioClips);
            lastVideoClips = videoClips;
        }
    }
    burstcon::BurstRequest req;
    req.index = burstcon::msToBurstIndex(tMs);
    req.quality = burstcon::BurstQuality{projectSettings.cacheResolution().width(), projectSettings.cacheResolution().height()};
    burstcon::VideoBurst out;
    if (root && root->videoBurst(req, out)) {
        std::lock_guard<std::mutex> lock(coverageMutex);
        const qint64 bucket = (tMs / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
        videoCoverageBuckets.insert(bucket);
        finalCoverageBuckets.insert(bucket);
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
    videoCoverageBuckets.clear();
    audioCoverageBuckets.clear();
    finalCoverageBuckets.clear();
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

QHash<QString, QVector<QPair<double, double>>> PlaybackPipeline::videoCacheSegments(const std::vector<Clip> &clips) const
{
    QHash<QString, QVector<QPair<double, double>>> out;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : clips) {
        QVector<QPair<double, double>> segs;
        const qint64 start = clip.startMs;
        const qint64 end = clip.startMs + std::max<qint64>(1, clip.durationMs);
        bool inRun = false;
        qint64 runStart = start;
        for (qint64 t = start; t < end; t += kVideoCoverageBucketMs) {
            const qint64 bucket = (t / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
            const bool hit = videoCoverageBuckets.find(bucket) != videoCoverageBuckets.end();
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
        for (qint64 t = start; t < end; t += kAudioCoverageBucketMs) {
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

QVector<QPair<double, double>> PlaybackPipeline::finalTimelineSegments(qint64 startMs, qint64 endMs) const
{
    QVector<QPair<double, double>> segs;
    if (endMs <= startMs) {
        return segs;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    bool inRun = false;
    qint64 runStart = startMs;
    for (qint64 t = startMs; t < endMs; t += kVideoCoverageBucketMs) {
        const qint64 bucket = (t / kVideoCoverageBucketMs) * kVideoCoverageBucketMs;
        const bool hit = finalCoverageBuckets.find(bucket) != finalCoverageBuckets.end();
        if (hit && !inRun) {
            inRun = true;
            runStart = t;
        } else if (!hit && inRun) {
            const double a = static_cast<double>(runStart - startMs) / static_cast<double>(std::max<qint64>(1, endMs - startMs));
            const double b = static_cast<double>(t - startMs) / static_cast<double>(std::max<qint64>(1, endMs - startMs));
            segs.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
            inRun = false;
        }
    }
    if (inRun) {
        const double a = static_cast<double>(runStart - startMs) / static_cast<double>(std::max<qint64>(1, endMs - startMs));
        segs.push_back({std::clamp(a, 0.0, 1.0), 1.0});
    }
    return segs;
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
}

void PlaybackPipeline::ensureAudioOutput()
{
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
    shutdownAudioOutput();

    if (audioSink && audioDevice) {
        return;
    }
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
    if (audioSink) {
        audioSink->stop();
        delete audioSink;
        audioSink = nullptr;
    }
    audioDevice = nullptr;
}

void PlaybackPipeline::pushAudioBurst(const burstcon::AudioBurst &burst)
{
    if (!audioDevice || burst.samples.empty()) {
        ensureAudioOutput();
    }
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
            ensureAudioOutput();
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
