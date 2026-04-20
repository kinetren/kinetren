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

    void start(qint64 timelineMs);
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
    QHash<QString, QVector<QPair<double, double>>> videoCacheSegments(const std::vector<Clip> &clips) const;
    QHash<QString, QVector<QPair<double, double>>> audioCacheSegments(const std::vector<Clip> &clips) const;
    QVector<QPair<double, double>> finalTimelineSegments(qint64 startMs, qint64 endMs) const;
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
    void pushAudioBurst(const burstcon::AudioBurst &burst);
    void requestDraftFrameAsync(qint64 tMs, const std::vector<Clip> &videoClips);
    void draftWorkerLoop();
    void requestPrefetchAsync(qint64 startMs, qint64 endMs, bool allowWhileRunning);
    void prefetchWorkerLoop();

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
    std::unordered_set<qint64> videoCoverageBuckets;
    std::unordered_set<qint64> audioCoverageBuckets;
    std::unordered_set<qint64> finalCoverageBuckets;

    std::shared_ptr<burstcon::ConMediaPool> mediaPool;
    std::shared_ptr<burstcon::ConTrack> videoTrack;
    std::shared_ptr<burstcon::ConTrack> audioTrack;
    std::shared_ptr<burstcon::ConTrimmer> trimmer;
    std::shared_ptr<burstcon::IBurstcon> root;
    std::unique_ptr<burstcon::MonotonicPlaybackTimer> timer;
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

    QIODevice *audioDevice = nullptr;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    QAudioSink *audioSink = nullptr;
#else
    QAudioOutput *audioSink = nullptr;
#endif
};

#endif // PLAYBACKPIPELINE_H
