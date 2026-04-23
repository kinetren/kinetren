#ifndef PLAYBACKPIPELINE_H
#define PLAYBACKPIPELINE_H

#include <QHash>
#include <QImage>
#include <QPair>
#include <QVector>

#include <atomic>
#include <condition_variable>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <unordered_set>
#include <vector>

#include "burstcon.h"
#include "projectsettings.h"

class QIODevice;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
class QAudioSink;
#else
class QAudioOutput;
#endif

class PlaybackPipeline
{
public:
    struct Clip {
        QString sourcePath;
        qint64 startMs = 0;
        qint64 durationMs = 0;
        qint64 sourceInMs = 0;
    };
    struct PerformanceStats {
        double composeFps = 0.0;
        quint64 composedFrames = 0;
        quint64 resourceCacheHits = 0;
        quint64 resourceCacheMisses = 0;
        quint64 draftFramesDecoded = 0;
        quint64 resourceCacheEntries = 0;
        quint64 resourceCacheUsedBytes = 0;
        quint64 resourceCacheLimitBytes = 0;
        quint64 resourceCacheConfiguredMaxBytes = 0;
        quint64 resourceCacheEvictions = 0;
        quint64 finalViewCacheHits = 0;
        quint64 finalViewCacheMisses = 0;
        quint64 audioCacheHits = 0;
        quint64 audioCacheMisses = 0;
        quint64 audioCacheEntries = 0;
        quint64 avcppOpenCalls = 0;
        quint64 avcppPacketReads = 0;
        quint64 avcppVideoDecodeCalls = 0;
        quint64 avcppAudioDecodeCalls = 0;
    };

    explicit PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady);
    ~PlaybackPipeline();

    /// Same pool as tracks/prefetch; timeline strip previews reuse it to avoid a second file index + demuxers.
    std::shared_ptr<burstcon::ConMediaPool> sharedMediaPool() const { return mediaPool; }

    void start(qint64 timelineMs);
    /// Arm audio/callbacks/quality without starting the playback clock (used before head warmup).
    void preparePlaybackAt(qint64 startTimelineMs);
    /// Decode video+audio for [startMs, startMs+headMs) at project FPS to prime caches and coverage.
    void warmPlaybackHeadBlocking(qint64 startTimelineMs, qint64 headMs);
    /// Start monotonic timer and async prefetch (call after warmup).
    void beginClockAndPrefetch(qint64 startTimelineMs);
    /// True when timeline [startMs, startMs+headMs) already has dense video+final coverage (skip redundant warmup).
    bool isPlaybackHeadWarm(qint64 startMs, qint64 headMs) const;
    void stop();
    bool isRunning() const;
    void update(qint64 timelineMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips);
    void setPlaybackMode(burstcon::PlaybackMode mode, qint64 regionStartMs, qint64 regionEndMs);
    qint64 playbackPositionMs() const;
    void setDraftPreviewEnabled(bool enabled);
    QImage renderDraftFrame(qint64 timelineMs, const std::vector<Clip> &videoClips);
    void setProjectSettings(const ProjectSettings &settings);
    void resetProjectCache();
    void clearTimelineCoverage();
    PerformanceStats performanceSnapshot() const;
    /// Video: ConCache-backed (updates on LRU evict). Sliding = full timeline occupancy; head = sliding ∩ first ~1.5s of the playback region (composition time).
    QHash<QString, QVector<QPair<double, double>>> videoFrameSlidingSegments(const std::vector<Clip> &clips) const;
    QHash<QString, QVector<QPair<double, double>>> videoFrameHeadSegments(const std::vector<Clip> &clips) const;
    QVector<QPair<double, double>> timelineVideoSlidingSegments(qint64 viewStartMs, qint64 viewEndMs) const;
    QVector<QPair<double, double>> timelineVideoHeadSegments(qint64 viewStartMs, qint64 viewEndMs) const;
    QHash<QString, QVector<QPair<double, double>>> audioCacheSegments(const std::vector<Clip> &clips) const;
    static QString coverageClipKey(const Clip &clip);

private:
    struct DraftJob {
        qint64 timelineMs = 0;
        std::vector<Clip> videoClips;
        quint64 serial = 0;
    };
    struct PrefetchJob {
        qint64 startMs = 0;
        qint64 endMs = 0;
        quint64 serial = 0;
        bool allowWhileRunning = true;
    };

    void rebuildGraph(const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips);
    static bool clipsEqual(const std::vector<Clip> &a, const std::vector<Clip> &b);
    void ensureAudioOutput();
    void shutdownAudioOutput();
    void tearDownAudioLocked();
    void pushAudioBurst(const burstcon::AudioBurst &burst);
    void requestDraftFrameAsync(qint64 tMs, const std::vector<Clip> &videoClips);
    void draftWorkerLoop();
    void requestPrefetchAsync(qint64 startMs, qint64 endMs, bool allowWhileRunning);
    void prefetchWorkerLoop();
    void onVideoFrameCacheKeyChanged(bool inserted,
                                     const QString &mediaPath,
                                     burstcon::BurstIndex mediaBurstIndex,
                                     const burstcon::BurstQuality &q);
    void clearVideoFrameBucketSets();

    std::function<void(QImage &&)> onVideoFrameReady;
    mutable std::mutex mutex;
    ProjectSettings projectSettings;
    std::vector<Clip> lastVideoClips;
    std::vector<Clip> lastAudioClips;
    std::atomic_bool running{false};
    std::atomic<qint64> timelineMs{0};
    std::atomic<double> composeFps{0.0};
    std::atomic<quint64> composedFrames{0};
    std::atomic<qint64> lastPlaybackIndex{0};
    std::atomic_bool draftPreviewEnabled{true};
    std::atomic<quint64> latestDraftSerial{0};
    std::atomic<quint64> latestPrefetchSerial{0};
    std::atomic<qint64> lastPrefetchAnchorMs{-1};
    std::atomic<burstcon::BurstIndex> lastVideoRequestIndex{-1};
    std::atomic<burstcon::BurstIndex> lastAudioRequestIndex{-1};
    mutable std::mutex coverageMutex;
    std::unordered_set<qint64> audioCoverageBuckets;
    /// Composition-timeline ms buckets (kVideoCoverageBucketMs grid) where ConCache currently holds a video frame.
    std::unordered_set<qint64> timelineVideoSlidingBuckets;

    std::shared_ptr<burstcon::ConMediaPool> mediaPool;
    std::shared_ptr<burstcon::ConTrack> videoTrack;
    std::shared_ptr<burstcon::ConTrack> audioTrack;
    std::shared_ptr<burstcon::ConTrimmer> trimmer;
    std::shared_ptr<burstcon::IBurstcon> root;
    std::unique_ptr<burstcon::MonotonicPlaybackTimer> timer;
    burstcon::PlaybackMode playbackMode = burstcon::PlaybackMode::Free;
    qint64 playbackRegionStartMs = 0;
    qint64 playbackRegionEndMs = 0;
    std::mutex draftMutex;
    std::condition_variable draftCv;
    std::thread draftWorker;
    bool stopDraftWorker = false;
    bool draftJobPending = false;
    DraftJob pendingDraftJob;
    std::mutex prefetchMutex;
    std::condition_variable prefetchCv;
    std::thread prefetchWorker;
    bool stopPrefetchWorker = false;
    bool prefetchJobPending = false;
    PrefetchJob pendingPrefetchJob;

    mutable std::mutex audioMutex;
    QIODevice *audioDevice = nullptr;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    QAudioSink *audioSink = nullptr;
#else
    QAudioOutput *audioSink = nullptr;
#endif
};

#endif // PLAYBACKPIPELINE_H
