#include "playbackpipeline.h"

#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
#include <QAudioSink>
#include <QMediaDevices>
#else
#include <QAudioOutput>
#endif
#include <QAudioFormat>
#include <QIODevice>
#include <QPainter>
#include <QThreadPool>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <deque>
#include <limits>
#include <memory>
#include <thread>
#include <unordered_set>
#include <utility>

#include <avcpp/audioresampler.h>
#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/ffmpeg.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/videorescaler.h>

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr int kAudioBlockMs = 20;
constexpr int kAudioSamplesPerBlock = (kAudioSampleRate * kAudioBlockMs) / 1000;
constexpr size_t kAudioRingSlots = 12; // 12 * 20ms = 240ms
constexpr qint64 kAudioMaxAheadBlocks = static_cast<qint64>(kAudioRingSlots - 2);

constexpr int kVideoFps = 30;
constexpr int kVideoFrameMs = 1000 / kVideoFps;
constexpr size_t kVideoRingSlots = 7; // 7 * 33ms ~= 231ms
constexpr qint64 kVideoMaxAheadFrames = static_cast<qint64>(kVideoRingSlots - 2);
constexpr int kDraftFrameBucketMs = 120;
const av::Rational kMillisecondsTimebase{1, 1000};

qint64 timestampToMs(const av::Timestamp &ts)
{
    if (ts.isNoPts()) {
        return -1;
    }
    return ts.timestamp(kMillisecondsTimebase);
}
}

struct PlaybackPipeline::AudioRingBuffer {
    explicit AudioRingBuffer(size_t capacity)
        : keys(capacity, -1)
        , blocks(capacity, std::vector<int16_t>(kAudioSamplesPerBlock * kAudioChannels, 0))
    {
    }

    void setBlock(qint64 idx, const std::vector<int16_t> &samples)
    {
        const size_t slot = static_cast<size_t>(idx % static_cast<qint64>(blocks.size()));
        std::lock_guard<std::mutex> lock(mutex);
        keys[slot] = idx;
        blocks[slot] = samples;
    }

    bool getBlockExact(qint64 idx, std::vector<int16_t> &samples) const
    {
        const size_t slot = static_cast<size_t>(idx % static_cast<qint64>(blocks.size()));
        std::lock_guard<std::mutex> lock(mutex);
        if (keys[slot] != idx) {
            return false;
        }
        samples = blocks[slot];
        return !samples.empty();
    }

    mutable std::mutex mutex;
    std::vector<qint64> keys;
    std::vector<std::vector<int16_t>> blocks;
};

struct PlaybackPipeline::VideoRingBuffer {
    explicit VideoRingBuffer(size_t capacity)
        : slotFrameIndices(capacity, -1)
        , times(capacity, -1)
        , frames(capacity)
    {
    }

    void setReadCursorTime(qint64 timeMs)
    {
        const qint64 frameIndex = std::max<qint64>(0, timeMs / kVideoFrameMs);
        std::lock_guard<std::mutex> lock(mutex);
        readCursorFrameIndex = frameIndex;
    }

    bool setFrame(qint64 timeMs, const QImage &img)
    {
        std::lock_guard<std::mutex> lock(mutex);
        const qint64 frameIndex = std::max<qint64>(0, timeMs / kVideoFrameMs);
        const qint64 maxBufferedFrameIndex = readCursorFrameIndex + static_cast<qint64>(frames.size()) - 1;
        if (frameIndex < readCursorFrameIndex || frameIndex > maxBufferedFrameIndex) {
            // Ignore stale or too-far-ahead frames; producer wait logic will retry timing naturally.
            return false;
        }
        const size_t slot = static_cast<size_t>(frameIndex % static_cast<qint64>(frames.size()));
        const qint64 existingFrameIndex = slotFrameIndices[slot];
        if (existingFrameIndex > frameIndex) {
            // Avoid replacing a newer frame with an older one on wrap-around races.
            return false;
        }
        slotFrameIndices[slot] = frameIndex;
        times[slot] = timeMs;
        frames[slot] = img;
        return true;
    }

    bool getFrameAtOrBefore(qint64 timeMs, qint64 maxBackMs, qint64 &foundTimeMs, QImage &img) const
    {
        const qint64 targetFrameIndex = std::max<qint64>(0, timeMs / kVideoFrameMs);
        const qint64 lower = std::max<qint64>(0, timeMs - maxBackMs);
        std::lock_guard<std::mutex> lock(mutex);
        qint64 bestFrameIndex = -1;
        qint64 bestTime = -1;
        int bestIndex = -1;
        for (size_t i = 0; i < frames.size(); ++i) {
            const qint64 slotFrameIndex = slotFrameIndices[i];
            if (slotFrameIndex < 0 || slotFrameIndex > targetFrameIndex) {
                continue;
            }
            const qint64 t = times[i];
            if (t < lower || t > timeMs) {
                continue;
            }
            if (frames[i].isNull()) {
                continue;
            }
            if (slotFrameIndex > bestFrameIndex || (slotFrameIndex == bestFrameIndex && t > bestTime)) {
                bestFrameIndex = slotFrameIndex;
                bestTime = t;
                bestIndex = static_cast<int>(i);
            }
        }
        if (bestIndex < 0) {
            return false;
        }
        foundTimeMs = bestTime;
        img = frames[static_cast<size_t>(bestIndex)];
        return true;
    }

    mutable std::mutex mutex;
    qint64 readCursorFrameIndex = 0;
    std::vector<qint64> slotFrameIndices;
    std::vector<qint64> times;
    std::vector<QImage> frames;
};

struct PlaybackPipeline::AudioWorker {
    std::string key;
    Clip clip;
    qint64 startOffsetMs = 0;
    qint64 timelineStartMs = 0;
    int generation = 0;
    std::atomic<qint64> producedBlockIndex{-1};
    std::shared_ptr<std::atomic_bool> stopFlag;
    std::shared_ptr<AudioRingBuffer> ring;
};

struct PlaybackPipeline::VideoWorker {
    std::string key;
    Clip clip;
    qint64 startOffsetMs = 0;
    qint64 playbackStartMs = 0;
    std::atomic<int> frameDurationUs{33333};
    std::atomic<qint64> consumedTimeMs{0};
    std::atomic<qint64> producedTimeMs{-1};
    int generation = 0;
    std::shared_ptr<std::atomic_bool> stopFlag;
    std::shared_ptr<VideoRingBuffer> ring;
};

struct PlaybackPipeline::AudioBlockCache {
    struct Entry {
        std::vector<int16_t> samples;
        std::list<std::string>::iterator lruIt;
    };

    struct Stats {
        quint64 entries = 0;
        quint64 hits = 0;
        quint64 misses = 0;
    };

    bool get(const std::string &key, std::vector<int16_t> &samples)
    {
        std::lock_guard<std::mutex> lock(mutex);
        auto it = entries.find(key);
        if (it == entries.end()) {
            ++missCount;
            return false;
        }
        lru.erase(it->second.lruIt);
        lru.push_front(key);
        it->second.lruIt = lru.begin();
        samples = it->second.samples;
        ++hitCount;
        return true;
    }

    void put(const std::string &key, const std::vector<int16_t> &samples)
    {
        if (samples.empty()) {
            return;
        }
        std::lock_guard<std::mutex> lock(mutex);
        auto existing = entries.find(key);
        if (existing != entries.end()) {
            lru.erase(existing->second.lruIt);
            entries.erase(existing);
        }
        lru.push_front(key);
        entries.emplace(key, Entry{samples, lru.begin()});
        while (entries.size() > maxEntries && !lru.empty()) {
            const std::string evictKey = lru.back();
            lru.pop_back();
            entries.erase(evictKey);
        }
    }

    Stats stats() const
    {
        std::lock_guard<std::mutex> lock(mutex);
        return Stats{static_cast<quint64>(entries.size()), hitCount, missCount};
    }

    mutable std::mutex mutex;
    std::unordered_map<std::string, Entry> entries;
    std::list<std::string> lru;
    size_t maxEntries = 24000;
    quint64 hitCount = 0;
    quint64 missCount = 0;
};

PlaybackPipeline::PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady)
    : onVideoFrameReady(std::move(onVideoFrameReady))
{
    audioBlockCache = std::make_unique<AudioBlockCache>();
    finalViewCache.setConfiguredMaxBytes(512ULL * 1024ULL * 1024ULL);
}

PlaybackPipeline::~PlaybackPipeline()
{
    stop();
}

void PlaybackPipeline::start(qint64 startTimelineMs)
{
    // If playback is already running, stop existing mixer/composer threads first.
    // Joining live threads from the UI thread would otherwise block loop restarts.
    running.store(false);
    ++generation;
    if (audioThread.joinable()) {
        audioThread.join();
    }
    if (videoThread.joinable()) {
        videoThread.join();
    }
    {
        std::lock_guard<std::mutex> lock(audioMutex);
        for (auto &it : audioWorkers) {
            it.second->stopFlag->store(true);
        }
        audioWorkers.clear();
    }
    {
        std::lock_guard<std::mutex> lock(videoMutex);
        for (auto &it : videoWorkers) {
            it.second->stopFlag->store(true);
        }
        videoWorkers.clear();
    }

    timelineMs.store(startTimelineMs);
    ++generation;
    running.store(true);
    ensureAudioOutput();
    nextAudioBlock = startTimelineMs / kAudioBlockMs;
    nextVideoFrame = startTimelineMs / kVideoFrameMs;
    consumedAudioBlock.store(nextAudioBlock);
    consumedVideoFrame.store(nextVideoFrame);

    audioThread = std::thread([this]() {
        audioMixLoop();
    });
    videoThread = std::thread([this]() {
        videoComposeLoop();
    });
}

void PlaybackPipeline::stop()
{
    running.store(false);
    ++generation;
    if (audioThread.joinable()) {
        audioThread.join();
    }
    if (videoThread.joinable()) {
        videoThread.join();
    }
    {
        std::lock_guard<std::mutex> lock(audioMutex);
        for (auto &it : audioWorkers) {
            it.second->stopFlag->store(true);
        }
        audioWorkers.clear();
    }
    {
        std::lock_guard<std::mutex> lock(videoMutex);
        for (auto &it : videoWorkers) {
            it.second->stopFlag->store(true);
        }
        videoWorkers.clear();
    }
    shutdownAudioOutput();
}

void PlaybackPipeline::update(qint64 tMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips)
{
    timelineMs.store(tMs);
    if (!running.load()) {
        if (onVideoFrameReady) {
            onVideoFrameReady(composeDraftFrameAt(tMs, videoClips));
        }
        return;
    }
    reconcileAudioWorkers(audioClips);
    reconcileVideoWorkers(videoClips);
}

qint64 PlaybackPipeline::playbackPositionMs() const
{
    if (!running.load()) {
        return timelineMs.load();
    }
    return consumedAudioBlock.load() * kAudioBlockMs;
}

void PlaybackPipeline::setDraftPreviewEnabled(bool enabled)
{
    draftPreviewEnabled.store(enabled);
}

QImage PlaybackPipeline::renderDraftFrame(qint64 timelineMs, const std::vector<Clip> &videoClips)
{
    return composeDraftFrameAt(timelineMs, videoClips);
}

void PlaybackPipeline::setProjectSettings(const ProjectSettings &settings)
{
    std::lock_guard<std::mutex> lock(settingsMutex);
    projectSettings = settings;
}

PlaybackPipeline::PerformanceStats PlaybackPipeline::performanceSnapshot() const
{
    PerformanceStats stats;
    stats.composeFps = composeFps.load();
    stats.composedFrames = composedFrames.load();
    stats.draftFramesDecoded = draftFramesDecoded.load();
    const auto cstats = frameCache.stats();
    stats.resourceCacheHits = cstats.hits;
    stats.resourceCacheMisses = cstats.misses;
    stats.resourceCacheEntries = cstats.entries;
    stats.resourceCacheUsedBytes = cstats.usedBytes;
    stats.resourceCacheLimitBytes = cstats.limitBytes;
    stats.resourceCacheConfiguredMaxBytes = cstats.configuredMaxBytes;
    stats.resourceCacheEvictions = cstats.evictions;
    stats.finalViewCacheHits = finalViewCacheHits.load();
    stats.finalViewCacheMisses = finalViewCacheMisses.load();
    const auto astats = audioBlockCache->stats();
    stats.audioCacheHits = astats.hits;
    stats.audioCacheMisses = astats.misses;
    stats.audioCacheEntries = astats.entries;
    stats.avcppOpenCalls = avcppOpenCalls.load();
    stats.avcppPacketReads = avcppPacketReads.load();
    stats.avcppVideoDecodeCalls = avcppVideoDecodeCalls.load();
    stats.avcppAudioDecodeCalls = avcppAudioDecodeCalls.load();
    return stats;
}

QHash<QString, QVector<QPair<double, double>>> PlaybackPipeline::videoCacheSegments(const std::vector<Clip> &clips) const
{
    QHash<QString, QVector<QPair<double, double>>> segmentsByClip;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : clips) {
        const std::string source = clip.sourcePath.toStdString();
        auto it = videoBucketsBySource.find(source);
        if (it == videoBucketsBySource.end()) {
            segmentsByClip.insert(coverageClipKey(clip), {});
            continue;
        }
        const qint64 start = clip.sourceInMs;
        const qint64 end = clip.sourceInMs + std::max<qint64>(1, clip.durationMs);
        QVector<QPair<double, double>> segments;
        bool inRun = false;
        qint64 runStart = start;
        for (qint64 t = start; t < end; t += kDraftFrameBucketMs) {
            const qint64 bucket = (t / kDraftFrameBucketMs) * kDraftFrameBucketMs;
            const bool hasBucket = (it->second.find(bucket) != it->second.end());
            if (hasBucket && !inRun) {
                inRun = true;
                runStart = t;
            } else if (!hasBucket && inRun) {
                const double a = static_cast<double>(runStart - start) / static_cast<double>(std::max<qint64>(1, end - start));
                const double b = static_cast<double>(t - start) / static_cast<double>(std::max<qint64>(1, end - start));
                segments.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
                inRun = false;
            }
        }
        if (inRun) {
            const double a = static_cast<double>(runStart - start) / static_cast<double>(std::max<qint64>(1, end - start));
            segments.push_back({std::clamp(a, 0.0, 1.0), 1.0});
        }
        segmentsByClip.insert(coverageClipKey(clip), segments);
    }
    return segmentsByClip;
}

QHash<QString, QVector<QPair<double, double>>> PlaybackPipeline::audioCacheSegments(const std::vector<Clip> &clips) const
{
    QHash<QString, QVector<QPair<double, double>>> segmentsByClip;
    std::lock_guard<std::mutex> lock(coverageMutex);
    for (const auto &clip : clips) {
        const std::string source = clip.sourcePath.toStdString();
        auto it = audioBucketsBySource.find(source);
        if (it == audioBucketsBySource.end()) {
            segmentsByClip.insert(coverageClipKey(clip), {});
            continue;
        }
        const qint64 start = clip.sourceInMs;
        const qint64 end = clip.sourceInMs + std::max<qint64>(1, clip.durationMs);
        QVector<QPair<double, double>> segments;
        bool inRun = false;
        qint64 runStart = start;
        for (qint64 t = start; t < end; t += kAudioBlockMs) {
            const qint64 bucket = (t / kAudioBlockMs) * kAudioBlockMs;
            const bool hasBucket = (it->second.find(bucket) != it->second.end());
            if (hasBucket && !inRun) {
                inRun = true;
                runStart = t;
            } else if (!hasBucket && inRun) {
                const double a = static_cast<double>(runStart - start) / static_cast<double>(std::max<qint64>(1, end - start));
                const double b = static_cast<double>(t - start) / static_cast<double>(std::max<qint64>(1, end - start));
                segments.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
                inRun = false;
            }
        }
        if (inRun) {
            const double a = static_cast<double>(runStart - start) / static_cast<double>(std::max<qint64>(1, end - start));
            segments.push_back({std::clamp(a, 0.0, 1.0), 1.0});
        }
        segmentsByClip.insert(coverageClipKey(clip), segments);
    }
    return segmentsByClip;
}

QVector<QPair<double, double>> PlaybackPipeline::finalTimelineSegments(qint64 startMs, qint64 endMs) const
{
    QVector<QPair<double, double>> segments;
    if (endMs <= startMs) {
        return segments;
    }
    std::lock_guard<std::mutex> lock(coverageMutex);
    bool inRun = false;
    qint64 runStart = startMs;
    for (qint64 t = std::max<qint64>(0, startMs); t < endMs; t += kVideoFrameMs) {
        const qint64 frameIdx = t / kVideoFrameMs;
        const bool hasFrame = (finalTimelineFrames.find(frameIdx) != finalTimelineFrames.end());
        if (hasFrame && !inRun) {
            inRun = true;
            runStart = t;
        } else if (!hasFrame && inRun) {
            const double a = static_cast<double>(runStart - startMs) / static_cast<double>(std::max<qint64>(1, endMs - startMs));
            const double b = static_cast<double>(t - startMs) / static_cast<double>(std::max<qint64>(1, endMs - startMs));
            segments.push_back({std::clamp(a, 0.0, 1.0), std::clamp(b, 0.0, 1.0)});
            inRun = false;
        }
    }
    if (inRun) {
        const double a = static_cast<double>(runStart - startMs) / static_cast<double>(std::max<qint64>(1, endMs - startMs));
        segments.push_back({std::clamp(a, 0.0, 1.0), 1.0});
    }
    return segments;
}

QString PlaybackPipeline::coverageClipKey(const Clip &clip)
{
    return QStringLiteral("%1|%2|%3|%4")
        .arg(clip.sourcePath)
        .arg(clip.startMs)
        .arg(clip.durationMs)
        .arg(clip.sourceInMs);
}

std::string PlaybackPipeline::clipKey(const Clip &clip)
{
    return (clip.sourcePath + "::" + QString::number(clip.startMs) + "::" +
            QString::number(clip.durationMs) + "::" + QString::number(clip.sourceInMs))
        .toStdString();
}

std::string PlaybackPipeline::finalViewKey(qint64 timelineMs, const std::vector<std::shared_ptr<VideoWorker>> &workers)
{
    QString key = QStringLiteral("final::%1").arg(timelineMs / kVideoFrameMs);
    for (const auto &worker : workers) {
        if (!worker) {
            continue;
        }
        key += QStringLiteral("|%1@%2")
                   .arg(QString::fromStdString(worker->key))
                   .arg(std::max<qint64>(0, timelineMs - worker->playbackStartMs) / kVideoFrameMs);
    }
    return key.toStdString();
}

std::string PlaybackPipeline::audioBlockKey(const QString &sourcePath, qint64 sourceTimeMs)
{
    const qint64 bucketMs = std::max<qint64>(0, (sourceTimeMs / kAudioBlockMs) * kAudioBlockMs);
    return (sourcePath + "::audio::" + QString::number(bucketMs)).toStdString();
}

std::string PlaybackPipeline::videoFrameKey(const QString &sourcePath, qint64 sourceTimeMs, bool draft)
{
    const qint64 bucketMs = std::max<qint64>(0, (sourceTimeMs / kDraftFrameBucketMs) * kDraftFrameBucketMs);
    return (sourcePath + "::" + QString::number(bucketMs) + "::" + (draft ? "draft" : "full")).toStdString();
}

void PlaybackPipeline::markResourceBucket(const QString &sourcePath, qint64 bucketMs)
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    videoBucketsBySource[sourcePath.toStdString()].insert(bucketMs);
}

void PlaybackPipeline::markAudioBucket(const QString &sourcePath, qint64 bucketMs)
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    audioBucketsBySource[sourcePath.toStdString()].insert(bucketMs);
}

void PlaybackPipeline::markFinalTimelineFrame(qint64 timelineMs)
{
    std::lock_guard<std::mutex> lock(coverageMutex);
    finalTimelineFrames.insert(std::max<qint64>(0, timelineMs / kVideoFrameMs));
}

void PlaybackPipeline::ensureAudioOutput()
{
    if (audioSink && audioDevice) {
        return;
    }
    QAudioFormat format;
    format.setSampleRate(kAudioSampleRate);
    format.setChannelCount(kAudioChannels);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    format.setSampleFormat(QAudioFormat::Int16);
    audioSink = new QAudioSink(QMediaDevices::defaultAudioOutput(), format);
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

void PlaybackPipeline::reconcileAudioWorkers(const std::vector<Clip> &clips)
{
    const qint64 nowMs = timelineMs.load();
    std::unordered_set<std::string> active;
    for (const auto &clip : clips) {
        const auto key = clipKey(clip);
        active.insert(key);
        bool exists = false;
        {
            std::lock_guard<std::mutex> lock(audioMutex);
            exists = (audioWorkers.find(key) != audioWorkers.end());
        }
        if (!exists) {
            startAudioWorker(clip, nowMs);
        }
    }

    std::lock_guard<std::mutex> lock(audioMutex);
    for (auto it = audioWorkers.begin(); it != audioWorkers.end();) {
        if (active.find(it->first) != active.end()) {
            ++it;
            continue;
        }
        it->second->stopFlag->store(true);
        it = audioWorkers.erase(it);
    }
}

void PlaybackPipeline::reconcileVideoWorkers(const std::vector<Clip> &clips)
{
    const qint64 nowMs = timelineMs.load();
    std::unordered_set<std::string> active;
    for (const auto &clip : clips) {
        const auto key = clipKey(clip);
        active.insert(key);
        bool exists = false;
        {
            std::lock_guard<std::mutex> lock(videoMutex);
            exists = (videoWorkers.find(key) != videoWorkers.end());
        }
        if (!exists) {
            startVideoWorker(clip, nowMs);
        }
    }

    std::lock_guard<std::mutex> lock(videoMutex);
    for (auto it = videoWorkers.begin(); it != videoWorkers.end();) {
        if (active.find(it->first) != active.end()) {
            ++it;
            continue;
        }
        it->second->stopFlag->store(true);
        it = videoWorkers.erase(it);
    }
}

void PlaybackPipeline::startAudioWorker(const Clip &clip, qint64 timelineNowMs)
{
    auto worker = std::make_shared<AudioWorker>();
    worker->key = clipKey(clip);
    worker->clip = clip;
    worker->timelineStartMs = std::max<qint64>(clip.startMs, timelineNowMs);
    worker->startOffsetMs = clip.sourceInMs + std::max<qint64>(0, worker->timelineStartMs - clip.startMs);
    worker->generation = generation.load();
    worker->stopFlag = std::make_shared<std::atomic_bool>(false);
    worker->ring = std::make_shared<AudioRingBuffer>(kAudioRingSlots);

    const auto key = worker->key;
    {
        std::lock_guard<std::mutex> lock(audioMutex);
        audioWorkers[key] = worker;
    }

    QThreadPool::globalInstance()->start([this, worker]() {
        (void)decodeAudioWorker(*worker);
    });
}

void PlaybackPipeline::startVideoWorker(const Clip &clip, qint64 timelineNowMs)
{
    auto worker = std::make_shared<VideoWorker>();
    worker->key = clipKey(clip);
    worker->clip = clip;
    worker->startOffsetMs = clip.sourceInMs + std::max<qint64>(0, timelineNowMs - clip.startMs);
    worker->playbackStartMs = clip.startMs;
    worker->consumedTimeMs.store(0);
    worker->generation = generation.load();
    worker->stopFlag = std::make_shared<std::atomic_bool>(false);
    worker->ring = std::make_shared<VideoRingBuffer>(kVideoRingSlots);

    const auto key = worker->key;
    {
        std::lock_guard<std::mutex> lock(videoMutex);
        videoWorkers[key] = worker;
    }

    QThreadPool::globalInstance()->start([this, worker]() {
        (void)decodeVideoWorker(*worker);
    });
}

void PlaybackPipeline::audioMixLoop()
{
    const int samplesPerBlock = kAudioSamplesPerBlock * kAudioChannels;
    const auto blockDuration = std::chrono::milliseconds(kAudioBlockMs);
    auto nextWake = std::chrono::steady_clock::now();

    while (running.load()) {
        if (!audioDevice) {
            std::this_thread::sleep_for(std::chrono::milliseconds(5));
            continue;
        }

        std::vector<std::shared_ptr<AudioWorker>> workers;
        {
            std::lock_guard<std::mutex> lock(audioMutex);
            workers.reserve(audioWorkers.size());
            for (auto &it : audioWorkers) {
                workers.push_back(it.second);
            }
        }

        std::vector<int32_t> mix(samplesPerBlock, 0);
        const qint64 blockMs = nextAudioBlock * kAudioBlockMs;
        for (const auto &worker : workers) {
            if (blockMs < worker->clip.startMs || blockMs >= (worker->clip.startMs + worker->clip.durationMs)) {
                continue;
            }
            for (int wait = 0; wait < 6 && running.load(); ++wait) {
                if (worker->producedBlockIndex.load() >= nextAudioBlock) {
                    break;
                }
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
            }
            std::vector<int16_t> block;
            bool ready = worker->ring->getBlockExact(nextAudioBlock, block);
            if (!ready) {
                const qint64 startBlock = worker->timelineStartMs / kAudioBlockMs;
                const qint64 sourceTimeMs =
                    worker->startOffsetMs + std::max<qint64>(0, nextAudioBlock - startBlock) * kAudioBlockMs;
                ready = audioBlockCache->get(audioBlockKey(worker->clip.sourcePath, sourceTimeMs), block);
            }
            if (!ready || static_cast<int>(block.size()) < samplesPerBlock) {
                continue;
            }
            for (int i = 0; i < samplesPerBlock; ++i) {
                mix[i] += block[i];
            }
        }

        std::vector<int16_t> out(samplesPerBlock, 0);
        for (int i = 0; i < samplesPerBlock; ++i) {
            out[i] = static_cast<int16_t>(std::clamp<int32_t>(
                mix[i],
                std::numeric_limits<int16_t>::min(),
                std::numeric_limits<int16_t>::max()));
        }

        const char *pcm = reinterpret_cast<const char *>(out.data());
        int bytes = static_cast<int>(out.size() * sizeof(int16_t));
        int offset = 0;
        while (offset < bytes && running.load()) {
            const qint64 written = audioDevice->write(pcm + offset, bytes - offset);
            if (written < 0) {
                break;
            }
            if (written == 0) {
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
                continue;
            }
            offset += static_cast<int>(written);
        }

        ++nextAudioBlock;
        consumedAudioBlock.store(nextAudioBlock);
        nextWake += blockDuration;
        std::this_thread::sleep_until(nextWake);
    }
}

void PlaybackPipeline::videoComposeLoop()
{
    auto nextWake = std::chrono::steady_clock::now();
    auto fpsWindowStart = nextWake;
    quint64 fpsWindowFrames = 0;
    std::unordered_map<std::string, qint64> lastPresentedTimeByWorker;
    std::unordered_map<std::string, QImage> lastPresentedFrameByWorker;
    while (running.load()) {
        std::vector<std::shared_ptr<VideoWorker>> workers;
        {
            std::lock_guard<std::mutex> lock(videoMutex);
            workers.reserve(videoWorkers.size());
            for (auto &it : videoWorkers) {
                workers.push_back(it.second);
            }
        }
        std::sort(workers.begin(), workers.end(), [](const std::shared_ptr<VideoWorker> &a, const std::shared_ptr<VideoWorker> &b) {
            if (!a || !b) {
                return static_cast<bool>(a);
            }
            return a->key < b->key;
        });

        const qint64 audioMasterMs = consumedAudioBlock.load() * kAudioBlockMs;
        const std::string composedCacheKey = finalViewKey(audioMasterMs, workers);
        QImage composed;
        bool hasAny = false;
        int composeFrameDurationUs = 33333;
        {
            QImage cachedFinal;
            if (finalViewCache.get(composedCacheKey, cachedFinal) && !cachedFinal.isNull()) {
                ++finalViewCacheHits;
                composed = cachedFinal;
                hasAny = true;
            } else {
                ++finalViewCacheMisses;
            }
        }
        if (hasAny) {
            markFinalTimelineFrame(audioMasterMs);
            if (onVideoFrameReady) {
                onVideoFrameReady(std::move(composed));
                ++fpsWindowFrames;
                composedFrames.fetch_add(1);
            }
            const auto now = std::chrono::steady_clock::now();
            const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(now - fpsWindowStart).count();
            if (elapsed >= 1000) {
                const double seconds = static_cast<double>(elapsed) / 1000.0;
                composeFps.store((seconds > 0.0) ? (static_cast<double>(fpsWindowFrames) / seconds) : 0.0);
                fpsWindowFrames = 0;
                fpsWindowStart = now;
            }
            ++nextVideoFrame;
            consumedVideoFrame.store(nextVideoFrame);
            const int sleepUs = std::clamp(composeFrameDurationUs / 2, 4000, 16000);
            nextWake += std::chrono::microseconds(sleepUs);
            std::this_thread::sleep_until(nextWake);
            continue;
        }
        for (const auto &worker : workers) {
            if (audioMasterMs < worker->clip.startMs || audioMasterMs >= (worker->clip.startMs + worker->clip.durationMs)) {
                continue;
            }
            composeFrameDurationUs = std::min(composeFrameDurationUs, worker->frameDurationUs.load());
            const qint64 targetTimeMs = std::max<qint64>(0, audioMasterMs - worker->playbackStartMs);
            worker->consumedTimeMs.store(targetTimeMs);
            worker->ring->setReadCursorTime(targetTimeMs);
            for (int wait = 0; wait < 8 && running.load(); ++wait) {
                if (worker->producedTimeMs.load() >= targetTimeMs) {
                    break;
                }
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
            }
            QImage frame;
            qint64 foundTimeMs = -1;
            const bool ready = worker->ring->getFrameAtOrBefore(
                targetTimeMs,
                static_cast<qint64>(kVideoRingSlots * kVideoFrameMs),
                foundTimeMs,
                frame);
            const qint64 lastPresentedTime =
                lastPresentedTimeByWorker.count(worker->key) ? lastPresentedTimeByWorker[worker->key] : -1;

            if (!ready || frame.isNull()) {
                const qint64 sourceTimeMs = worker->clip.sourceInMs + targetTimeMs;
                const std::string cacheKey = videoFrameKey(worker->clip.sourcePath, sourceTimeMs, draftPreviewEnabled.load());
                (void)frameCache.get(cacheKey, frame);
                if (frame.isNull()) {
                    auto it = lastPresentedFrameByWorker.find(worker->key);
                    if (it == lastPresentedFrameByWorker.end() || it->second.isNull()) {
                        continue;
                    }
                    frame = it->second;
                }
            } else if (foundTimeMs < lastPresentedTime) {
                auto it = lastPresentedFrameByWorker.find(worker->key);
                if (it == lastPresentedFrameByWorker.end() || it->second.isNull()) {
                    continue;
                }
                frame = it->second;
            } else {
                lastPresentedTimeByWorker[worker->key] = foundTimeMs;
                lastPresentedFrameByWorker[worker->key] = frame;
            }

            if (!hasAny) {
                composed = frame.convertToFormat(QImage::Format_RGB888);
                hasAny = true;
                continue;
            }

            QPainter p(&composed);
            p.drawImage(composed.rect(), frame);
        }

        if (hasAny && onVideoFrameReady) {
            finalViewCache.put(composedCacheKey, composed);
            markFinalTimelineFrame(audioMasterMs);
            onVideoFrameReady(std::move(composed));
            ++fpsWindowFrames;
            composedFrames.fetch_add(1);
        }

        const auto now = std::chrono::steady_clock::now();
        const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(now - fpsWindowStart).count();
        if (elapsed >= 1000) {
            const double seconds = static_cast<double>(elapsed) / 1000.0;
            composeFps.store((seconds > 0.0) ? (static_cast<double>(fpsWindowFrames) / seconds) : 0.0);
            fpsWindowFrames = 0;
            fpsWindowStart = now;
        }

        ++nextVideoFrame;
        consumedVideoFrame.store(nextVideoFrame);
        const int sleepUs = std::clamp(composeFrameDurationUs / 2, 4000, 16000);
        nextWake += std::chrono::microseconds(sleepUs);
        std::this_thread::sleep_until(nextWake);
    }
}

int PlaybackPipeline::decodeAudioWorker(AudioWorker &worker)
{
    using namespace av;

    const qint64 startBlock = worker.timelineStartMs / kAudioBlockMs;
    const qint64 clipEndMs = worker.clip.startMs + worker.clip.durationMs;
    const qint64 clipEndBlock = (clipEndMs + kAudioBlockMs - 1) / kAudioBlockMs;
    bool fullCached = true;
    for (qint64 block = startBlock; block < clipEndBlock; ++block) {
        std::vector<int16_t> cached;
        const qint64 sourceTimeMs = worker.startOffsetMs + (block - startBlock) * kAudioBlockMs;
        if (!audioBlockCache->get(audioBlockKey(worker.clip.sourcePath, sourceTimeMs), cached)) {
            fullCached = false;
            break;
        }
    }
    if (fullCached && clipEndBlock > startBlock) {
        return 0;
    }

    std::error_code ec;
    FormatContext input;
    avcppOpenCalls.fetch_add(1);
    input.openInput(worker.clip.sourcePath.toStdString(), ec);
    if (ec) {
        return 1;
    }
    input.findStreamInfo(ec);
    if (ec) {
        return 1;
    }

    int audioStream = -1;
    Stream stream;
    for (size_t i = 0; i < input.streamsCount(); ++i) {
        auto st = input.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_AUDIO) {
            audioStream = static_cast<int>(i);
            stream = st;
            break;
        }
    }
    if (stream.isNull()) {
        return 1;
    }

    AudioDecoderContext dec(stream);
    Codec codec = findDecodingCodec(dec.raw()->codec_id);
    dec.setCodec(codec);
    dec.open(ec);
    if (ec) {
        return 1;
    }

    AudioResampler resampler(AV_CH_LAYOUT_STEREO,
                             kAudioSampleRate,
                             AV_SAMPLE_FMT_S16,
                             dec.channelLayout(),
                             dec.sampleRate(),
                             dec.sampleFormat(),
                             ec);
    if (ec) {
        return 1;
    }

    const int blockInterleaved = kAudioSamplesPerBlock * kAudioChannels;
    std::deque<int16_t> pending;
    qint64 nextBlock = startBlock;
    bool started = false;

    const auto popToRing = [&](bool getAll) -> int {
        while (true) {
            AudioSamples out(AV_SAMPLE_FMT_S16, 1024, AV_CH_LAYOUT_STEREO, kAudioSampleRate);
            const bool ok = resampler.pop(out, getAll, ec);
            if (ec) {
                return 1;
            }
            if (!ok || !out) {
                return 0;
            }
            const int count = out.samplesCount() * out.channelsCount();
            const int16_t *pcm = reinterpret_cast<const int16_t *>(out.data(0));
            for (int i = 0; i < count; ++i) {
                pending.push_back(pcm[i]);
            }
            while (static_cast<int>(pending.size()) >= blockInterleaved) {
                if (worker.stopFlag->load() || worker.generation != generation.load()) {
                    return 0;
                }
                while (worker.generation == generation.load() && !worker.stopFlag->load()) {
                    const qint64 timelineBlock = consumedAudioBlock.load();
                    const qint64 maxAhead = kAudioMaxAheadBlocks;
                    if (nextBlock <= timelineBlock + maxAhead) {
                        break;
                    }
                    std::this_thread::sleep_for(std::chrono::milliseconds(2));
                }
                std::vector<int16_t> block(blockInterleaved);
                for (int i = 0; i < blockInterleaved; ++i) {
                    block[i] = pending.front();
                    pending.pop_front();
                }
                const qint64 sourceTimeMs = worker.startOffsetMs + (nextBlock - startBlock) * kAudioBlockMs;
                audioBlockCache->put(audioBlockKey(worker.clip.sourcePath, sourceTimeMs), block);
                markAudioBucket(worker.clip.sourcePath, std::max<qint64>(0, (sourceTimeMs / kAudioBlockMs) * kAudioBlockMs));
                worker.ring->setBlock(nextBlock, block);
                worker.producedBlockIndex.store(nextBlock);
                ++nextBlock;
            }
        }
    };

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        Packet pkt = input.readPacket(ec);
        avcppPacketReads.fetch_add(1);
        if (!pkt) {
            // End of stream or temporary no packet; stop decode loop and flush decoder.
            break;
        }
        if (ec) {
            // Non-fatal packet read errors should not kill the track worker.
            ec.clear();
            continue;
        }
        if (pkt.streamIndex() != audioStream) {
            continue;
        }
        avcppAudioDecodeCalls.fetch_add(1);
        AudioSamples samples = dec.decode(pkt, ec);
        if (ec) {
            return 1;
        }
        if (!samples) {
            continue;
        }
        if (!samples.pts().isNoPts()) {
            const qint64 ptsMs = timestampToMs(samples.pts());
            if (!started && ptsMs < worker.startOffsetMs) {
                continue;
            }
        }
        started = true;
        resampler.push(samples, ec);
        if (ec) {
            return 1;
        }
        if (popToRing(false) != 0) {
            return 1;
        }
    }

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        avcppAudioDecodeCalls.fetch_add(1);
        AudioSamples samples = dec.decode(Packet(), ec);
        if (ec) {
            ec.clear();
            break;
        }
        if (!samples) {
            break;
        }
        resampler.push(samples, ec);
        if (ec || popToRing(false) != 0) {
            break;
        }
    }
    (void)popToRing(true);

    return 0;
}

int PlaybackPipeline::decodeVideoWorker(VideoWorker &worker)
{
    using namespace av;
    ProjectSettings settingsCopy;
    {
        std::lock_guard<std::mutex> lock(settingsMutex);
        settingsCopy = projectSettings;
    }
    const qint64 seekLookbackMs = std::max<qint64>(1, settingsCopy.effectiveDecodeSeekLookbackMs());

    const qint64 clipStartSourceMs = worker.startOffsetMs;
    const qint64 clipEndSourceMs = worker.clip.sourceInMs + worker.clip.durationMs;
    bool fullCached = true;
    for (qint64 t = clipStartSourceMs; t < clipEndSourceMs; t += kDraftFrameBucketMs) {
        if (!frameCache.contains(videoFrameKey(worker.clip.sourcePath, t, draftPreviewEnabled.load()))) {
            fullCached = false;
            break;
        }
    }
    if (fullCached && clipEndSourceMs > clipStartSourceMs) {
        return 0;
    }

    std::error_code ec;
    FormatContext input;
    avcppOpenCalls.fetch_add(1);
    input.openInput(worker.clip.sourcePath.toStdString(), ec);
    if (ec) {
        return 1;
    }
    input.findStreamInfo(ec);
    if (ec) {
        return 1;
    }

    int videoStream = -1;
    Stream stream;
    for (size_t i = 0; i < input.streamsCount(); ++i) {
        auto st = input.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_VIDEO) {
            videoStream = static_cast<int>(i);
            stream = st;
            break;
        }
    }
    if (stream.isNull()) {
        return 1;
    }

    VideoDecoderContext dec(stream);
    Codec codec = findDecodingCodec(dec.raw()->codec_id);
    dec.setCodec(codec);
    dec.setRefCountedFrames(true);
    Codec codecCtx{};
    dec.open({{"threads", "1"}}, codecCtx, ec);
    if (ec) {
        return 1;
    }

    const auto avgRate = stream.averageFrameRate();
    double fps = avgRate.getDouble();
    if (fps < 1.0) {
        fps = stream.frameRate().getDouble();
    }
    if (fps < 1.0) {
        fps = 30.0;
    }
    worker.frameDurationUs.store(static_cast<int>(std::llround(1000000.0 / fps)));

    // Jump close to requested timeline position to avoid decoding from file start.
    if (worker.startOffsetMs > 0 && input.seekable()) {
        const qint64 seekMs = std::max<qint64>(0, worker.startOffsetMs - seekLookbackMs);
        input.seek(Timestamp(std::chrono::milliseconds(seekMs)),
                   static_cast<size_t>(videoStream),
                   false,
                   ec);
        if (ec) {
            // Keep playback functional even when a demuxer doesn't support stream seek well.
            ec.clear();
        }
    }

    VideoRescaler rescaler(dec.width(), dec.height(), AV_PIX_FMT_RGB24);
    qint64 lastPublishedTimeMs = -1;
    bool started = false;
    const auto applyCacheQualityScale = [this](QImage &frame) {
        if (frame.isNull()) {
            return;
        }
        ProjectSettings settingsCopy;
        {
            std::lock_guard<std::mutex> lock(settingsMutex);
            settingsCopy = projectSettings;
        }
        const QSize cacheResolution = settingsCopy.cacheResolution();
        if (cacheResolution.width() <= 0 || cacheResolution.height() <= 0) {
            return;
        }
        if (frame.width() == cacheResolution.width() && frame.height() == cacheResolution.height()) {
            return;
        }
        const Qt::TransformationMode mode =
            draftPreviewEnabled.load() ? Qt::FastTransformation : Qt::SmoothTransformation;
        frame = frame.scaled(cacheResolution, Qt::KeepAspectRatio, mode);
    };

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        Packet pkt = input.readPacket(ec);
        avcppPacketReads.fetch_add(1);
        if (!pkt) {
            // End of stream or temporary no packet; stop decode loop and flush decoder.
            break;
        }
        if (ec) {
            ec.clear();
            continue;
        }
        if (pkt.streamIndex() != videoStream) {
            continue;
        }

        avcppVideoDecodeCalls.fetch_add(1);
        VideoFrame frame = dec.decode(pkt, ec);
        if (ec) {
            return 1;
        }
        if (frame.isNull() || !frame.isComplete()) {
            continue;
        }

        qint64 frameMs = timestampToMs(frame.pts());
        if (frameMs < 0) {
            frameMs = (lastPublishedTimeMs >= 0) ? (lastPublishedTimeMs + kVideoFrameMs) : worker.startOffsetMs;
        }
        if (!started && frameMs < worker.startOffsetMs) {
            continue;
        }
        started = true;

        auto rgb = rescaler.rescale(frame);
        QImage img(rgb.width(), rgb.height(), QImage::Format_RGB888);
        const int dstStride = img.bytesPerLine();
        const int srcStride = rgb.raw()->linesize[0];
        const int copyStride = std::min(dstStride, srcStride);
        auto *src = rgb.data();
        auto *dst = img.bits();
        for (int y = 0; y < rgb.height(); ++y) {
            std::memcpy(dst + y * dstStride, src + y * srcStride, copyStride);
        }
        applyCacheQualityScale(img);

        const qint64 relTimeMs = std::max<qint64>(0, frameMs - worker.clip.sourceInMs);
        if (relTimeMs < lastPublishedTimeMs) {
            // Never publish older timestamps (prevents backward flicker).
            continue;
        }
        while (worker.generation == generation.load() && !worker.stopFlag->load()) {
            const qint64 consumedTimeMs = worker.consumedTimeMs.load();
            const qint64 maxAheadMs = static_cast<qint64>(kVideoRingSlots * kVideoFrameMs - kVideoFrameMs);
            if (relTimeMs <= consumedTimeMs + maxAheadMs) {
                break;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
        if (worker.ring->setFrame(relTimeMs, img)) {
            worker.producedTimeMs.store(relTimeMs);
            lastPublishedTimeMs = relTimeMs;
        }
        const qint64 sourceTimeMs = worker.clip.sourceInMs + relTimeMs;
        const qint64 bucketMs = std::max<qint64>(0, (sourceTimeMs / kDraftFrameBucketMs) * kDraftFrameBucketMs);
        const std::string cacheKey = videoFrameKey(worker.clip.sourcePath, sourceTimeMs, draftPreviewEnabled.load());
        frameCache.put(cacheKey, img);
        markResourceBucket(worker.clip.sourcePath, bucketMs);
    }

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        avcppVideoDecodeCalls.fetch_add(1);
        VideoFrame frame = dec.decode(Packet(), ec);
        if (ec) {
            ec.clear();
            break;
        }
        if (!frame || frame.isNull() || !frame.isComplete()) {
            break;
        }

        qint64 frameMs = timestampToMs(frame.pts());
        if (frameMs < 0) {
            frameMs = (lastPublishedTimeMs >= 0) ? (lastPublishedTimeMs + kVideoFrameMs) : worker.startOffsetMs;
        }
        if (frameMs < worker.startOffsetMs) {
            continue;
        }

        auto rgb = rescaler.rescale(frame);
        QImage img(rgb.width(), rgb.height(), QImage::Format_RGB888);
        const int dstStride = img.bytesPerLine();
        const int srcStride = rgb.raw()->linesize[0];
        const int copyStride = std::min(dstStride, srcStride);
        auto *src = rgb.data();
        auto *dst = img.bits();
        for (int y = 0; y < rgb.height(); ++y) {
            std::memcpy(dst + y * dstStride, src + y * srcStride, copyStride);
        }
        applyCacheQualityScale(img);

        const qint64 relTimeMs = std::max<qint64>(0, frameMs - worker.clip.sourceInMs);
        if (relTimeMs < lastPublishedTimeMs) {
            continue;
        }

        while (worker.generation == generation.load() && !worker.stopFlag->load()) {
            const qint64 consumedTimeMs = worker.consumedTimeMs.load();
            const qint64 maxAheadMs = static_cast<qint64>(kVideoRingSlots * kVideoFrameMs - kVideoFrameMs);
            if (relTimeMs <= consumedTimeMs + maxAheadMs) {
                break;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
        if (worker.ring->setFrame(relTimeMs, img)) {
            worker.producedTimeMs.store(relTimeMs);
            lastPublishedTimeMs = relTimeMs;
        }
        const qint64 sourceTimeMs = worker.clip.sourceInMs + relTimeMs;
        const qint64 bucketMs = std::max<qint64>(0, (sourceTimeMs / kDraftFrameBucketMs) * kDraftFrameBucketMs);
        const std::string cacheKey = videoFrameKey(worker.clip.sourcePath, sourceTimeMs, draftPreviewEnabled.load());
        frameCache.put(cacheKey, img);
        markResourceBucket(worker.clip.sourcePath, bucketMs);
    }

    return 0;
}

QImage PlaybackPipeline::composeDraftFrameAt(qint64 tMs, const std::vector<Clip> &videoClips)
{
    if (videoClips.empty()) {
        return QImage();
    }

    QString draftKey = QStringLiteral("final::draft::%1").arg(tMs / kVideoFrameMs);
    for (const auto &clip : videoClips) {
        draftKey += "|" + QString::fromStdString(clipKey(clip));
    }
    {
        QImage cached;
        if (finalViewCache.get(draftKey.toStdString(), cached) && !cached.isNull()) {
            ++finalViewCacheHits;
            return cached;
        }
        ++finalViewCacheMisses;
    }

    QImage composed;
    bool hasAny = false;
    for (const auto &clip : videoClips) {
        if (tMs < clip.startMs || tMs >= (clip.startMs + clip.durationMs)) {
            continue;
        }
        const qint64 sourceTimeMs = clip.sourceInMs + (tMs - clip.startMs);
        QImage frame = decodeVideoFrameAt(clip, sourceTimeMs);
        if (frame.isNull()) {
            continue;
        }
        if (!hasAny) {
            composed = frame.convertToFormat(QImage::Format_RGB888);
            hasAny = true;
            continue;
        }
        QPainter p(&composed);
        p.drawImage(composed.rect(), frame);
    }

    if (!hasAny) {
        return QImage();
    }
    finalViewCache.put(draftKey.toStdString(), composed);
    markFinalTimelineFrame(tMs);
    return composed;
}

QImage PlaybackPipeline::decodeVideoFrameAt(const Clip &clip, qint64 sourceTimeMs)
{
    using namespace av;
    ProjectSettings settingsCopy;
    {
        std::lock_guard<std::mutex> lock(settingsMutex);
        settingsCopy = projectSettings;
    }
    const qint64 seekLookbackMs = std::max<qint64>(1, settingsCopy.effectiveDecodeSeekLookbackMs());

    const qint64 bucketMs = std::max<qint64>(0, (sourceTimeMs / kDraftFrameBucketMs) * kDraftFrameBucketMs);
    const std::string cacheKey = videoFrameKey(clip.sourcePath, sourceTimeMs, draftPreviewEnabled.load());
    {
        QImage cached;
        if (frameCache.get(cacheKey, cached)) {
            return cached;
        }
    }

    std::error_code ec;
    FormatContext input;
    avcppOpenCalls.fetch_add(1);
    input.openInput(clip.sourcePath.toStdString(), ec);
    if (ec) {
        return QImage();
    }
    input.findStreamInfo(ec);
    if (ec) {
        return QImage();
    }

    int videoStream = -1;
    Stream stream;
    for (size_t i = 0; i < input.streamsCount(); ++i) {
        auto st = input.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_VIDEO) {
            videoStream = static_cast<int>(i);
            stream = st;
            break;
        }
    }
    if (stream.isNull() || videoStream < 0) {
        return QImage();
    }

    VideoDecoderContext dec(stream);
    Codec codec = findDecodingCodec(dec.raw()->codec_id);
    dec.setCodec(codec);
    dec.setRefCountedFrames(true);
    Codec codecCtx{};
    dec.open({{"threads", "1"}}, codecCtx, ec);
    if (ec) {
        return QImage();
    }

    const qint64 seekMs = std::max<qint64>(0, bucketMs - seekLookbackMs);
    if (input.seekable()) {
        input.seek(Timestamp(std::chrono::milliseconds(seekMs)),
                   static_cast<size_t>(videoStream),
                   false,
                   ec);
        if (ec) {
            ec.clear();
        }
    }

    VideoRescaler rescaler(dec.width(), dec.height(), AV_PIX_FMT_RGB24);
    QImage picked;
    qint64 pickedMs = -1;
    for (int tries = 0; tries < 600; ++tries) {
        Packet pkt = input.readPacket(ec);
        avcppPacketReads.fetch_add(1);
        if (ec || !pkt) {
            break;
        }
        if (pkt.streamIndex() != videoStream) {
            continue;
        }
        avcppVideoDecodeCalls.fetch_add(1);
        VideoFrame frame = dec.decode(pkt, ec);
        if (ec || frame.isNull() || !frame.isComplete()) {
            ec.clear();
            continue;
        }
        qint64 frameMs = timestampToMs(frame.pts());
        if (frameMs < 0) {
            frameMs = bucketMs;
        }
        if (frameMs < bucketMs) {
            continue;
        }

        auto rgb = rescaler.rescale(frame);
        QImage img(rgb.width(), rgb.height(), QImage::Format_RGB888);
        const int dstStride = img.bytesPerLine();
        const int srcStride = rgb.raw()->linesize[0];
        const int copyStride = std::min(dstStride, srcStride);
        auto *src = rgb.data();
        auto *dst = img.bits();
        for (int y = 0; y < rgb.height(); ++y) {
            std::memcpy(dst + y * dstStride, src + y * srcStride, copyStride);
        }
        picked = img;
        pickedMs = frameMs;
        break;
    }

    if (picked.isNull() || pickedMs < 0) {
        return QImage();
    }

    const QSize cacheResolution = settingsCopy.cacheResolution();
    if (cacheResolution.width() > 0 && cacheResolution.height() > 0 &&
        (picked.width() != cacheResolution.width() || picked.height() != cacheResolution.height())) {
        const Qt::TransformationMode mode =
            draftPreviewEnabled.load() ? Qt::FastTransformation : Qt::SmoothTransformation;
        picked = picked.scaled(cacheResolution, Qt::KeepAspectRatio, mode);
    }
    draftFramesDecoded.fetch_add(1);
    frameCache.put(cacheKey, picked);
    markResourceBucket(clip.sourcePath, bucketMs);
    return picked;
}
