#ifndef PLAYBACKPIPELINE_H
#define PLAYBACKPIPELINE_H

#include <QHash>
#include <QImage>
#include <QJsonArray>
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

#include "bc.h"
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
        /// Timeline video track index; lower = upper track in the UI (foreground when compositing).
        int trackIndex = 0;
        QJsonArray videoFxChain;
        QJsonArray audioFxChain;
        float audioLinearGain = 1.f;
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
    /// True when the output-aligned frame at `tMs` already has coverage (used for idle preview only).
    bool isScrubFrameWarm(qint64 tMs) const;
    void stop();
    bool isRunning() const;
    void update(qint64 timelineMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips);
    void setPlaybackMode(burstcon::PlaybackMode mode, qint64 regionStartMs, qint64 regionEndMs);
    qint64 playbackPositionMs() const;
    void setDraftPreviewEnabled(bool enabled);
    /// Master gain for preview/playback output (0..1, default 1).
    void setPreviewVolume(float linear01);
    float previewVolume() const;
    using AudioLevelsCallback = std::function<void(float leftDb, float rightDb)>;
    void setAudioLevelsCallback(AudioLevelsCallback cb);
    /// Opens preview audio output on the GUI thread (required before idle preview can be heard).
    void ensureIdlePreviewAudio();
    /// Scrub on the ruler while paused: same decode/audio path as playback, no clock advance.
    void beginScrubPlayback(qint64 timelineMs);
    void scrubPlaybackSeek(qint64 timelineMs);
    void endScrubPlayback();
    bool isScrubPlaybackActive() const { return scrubPlaybackActive.load(); }
    /// Qt6 pull-mode PCM device reads from the preview ring buffer.
    qint64 readAudioPcmForDevice(char *data, qint64 maxBytes);
    /// One composited burst at `timelineMs` for meter refresh while stopped/scrubbing.
    void requestMixedLevelsSampleAsync(qint64 timelineMs);
    QImage renderDraftFrame(qint64 timelineMs, const std::vector<Clip> &videoClips);
    void setProjectSettings(const ProjectSettings &settings);
    void resetProjectCache();
    void clearTimelineCoverage();
    PerformanceStats performanceSnapshot() const;
    /// Video: ConCache-backed (updates on LRU evict). Sliding = full timeline occupancy; head = sliding ∩ first ~1.5s of the playback region (composition time).
    QHash<QString, QVector<QPair<double, double>>> videoFrameSlidingSegments(const std::vector<Clip> &clips) const;
    QHash<QString, QVector<QPair<double, double>>> videoFrameHeadSegments(const std::vector<Clip> &clips) const;
    /// When `forwardAnchorMs >= 0`, the project strip only draws coverage at/after that time (lookahead).
    QVector<QPair<double, double>> timelineVideoSlidingSegments(qint64 viewStartMs,
                                                               qint64 viewEndMs,
                                                               qint64 forwardAnchorMs = -1) const;
    QVector<QPair<double, double>> timelineVideoHeadSegments(qint64 viewStartMs, qint64 viewEndMs) const;
    /// Composition-timeline audio ConCache coverage (cyan strip on the ruler).
    QVector<QPair<double, double>> timelineAudioSlidingSegments(qint64 viewStartMs,
                                                                qint64 viewEndMs,
                                                                qint64 forwardAnchorMs = -1) const;
    QHash<QString, QVector<QPair<double, double>>> audioCacheSegments(const std::vector<Clip> &clips) const;
    static QString coverageClipKey(const Clip &clip);

private:
    struct DraftJob {
        enum class Kind { IdlePreview, MixedLevelsSample };
        Kind kind = Kind::IdlePreview;
        qint64 timelineMs = 0;
        std::vector<Clip> videoClips;
    };
    static std::pair<float, float> peaksDbFromAudioBurst(const burstcon::AudioBurst &burst, float gainLinear);
    std::pair<float, float> sampleMixedLevelsDbAt(qint64 timelineMs) const;
    bool fetchMixedAudioBurstAt(qint64 timelineMs, burstcon::AudioBurst &out) const;
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
    void clearAudioPendingLocked();
    void notifyAudioPcmReady();
    /// Drain a few milliseconds of queued PCM without blocking the playback timer thread indefinitely.
    void flushAudioPendingToDevice();
    void requestIdlePreviewAsync(qint64 tMs, const std::vector<Clip> &videoClips);
    void draftWorkerLoop();
    void requestPrefetchAsync(qint64 startMs, qint64 endMs, bool allowWhileRunning);
    void prefetchWorkerLoop();
    void onVideoFrameCacheKeyChanged(bool inserted,
                                     const QString &mediaPath,
                                     burstcon::BurstIndex mediaBurstIndex,
                                     const burstcon::BurstQuality &q);
    void onAudioBurstCacheKeyChanged(bool inserted,
                                     const QString &mediaPath,
                                     burstcon::BurstIndex mediaBurstIndex);
    void clearVideoFrameBucketSets();

    std::function<void(QImage &&)> onVideoFrameReady;
    AudioLevelsCallback onAudioLevelsReady;
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
    std::atomic_bool scrubPlaybackActive{false};
    std::atomic<burstcon::BurstIndex> lastVideoRequestIndex{-1};
    std::atomic<burstcon::BurstIndex> lastAudioRequestIndex{-1};
    std::atomic<float> previewVolumeLinear{1.f};
    mutable std::mutex coverageMutex;
    std::unordered_set<qint64> audioCoverageBuckets;
    /// Composition-timeline ms buckets (kVideoCoverageBucketMs grid) where ConCache currently holds a video frame.
    std::unordered_set<qint64> timelineVideoSlidingBuckets;
    /// Buckets primed at the playback region start (warmup/prefetch); kept across LRU evicts so the yellow
    /// head strip does not vanish while sliding coverage is evicted on repeat.
    std::unordered_set<qint64> timelineVideoRegionHeadBuckets;

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
    /// PCM waiting for QIODevice::write (stereo int16 interleaved). Bounded so memory stays stable.
    std::vector<int16_t> audioPcmPending;
    QIODevice *audioDevice = nullptr;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    QAudioSink *audioSink = nullptr;
    class PreviewPcmIODevice *pcmFeedDevice = nullptr;
    QAudioFormat audioOutputFormat;
#else
    QAudioOutput *audioSink = nullptr;
#endif
};

#endif // PLAYBACKPIPELINE_H
