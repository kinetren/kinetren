#ifndef PLAYBACKPIPELINE_H
#define PLAYBACKPIPELINE_H

#include <QImage>
#include <QHash>
#include <QPair>
#include <QVector>
#include <QString>
#include <QtGlobal>
#include "framecache.h"
#include "projectsettings.h"

#include <atomic>
#include <cstdint>
#include <functional>
#include <list>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <thread>

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
    void update(qint64 timelineMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips);
    qint64 playbackPositionMs() const;
    void setDraftPreviewEnabled(bool enabled);
    QImage renderDraftFrame(qint64 timelineMs, const std::vector<Clip> &videoClips);
    void setProjectSettings(const ProjectSettings &settings);
    PerformanceStats performanceSnapshot() const;
    QHash<QString, QVector<QPair<double, double>>> videoCacheSegments(const std::vector<Clip> &clips) const;
    QHash<QString, QVector<QPair<double, double>>> audioCacheSegments(const std::vector<Clip> &clips) const;
    QVector<QPair<double, double>> finalTimelineSegments(qint64 startMs, qint64 endMs) const;
    static QString coverageClipKey(const Clip &clip);

private:
    struct AudioRingBuffer;
    struct VideoRingBuffer;
    struct AudioWorker;
    struct VideoWorker;
    struct AudioBlockCache;

    void ensureAudioOutput();
    void shutdownAudioOutput();

    void reconcileAudioWorkers(const std::vector<Clip> &clips);
    void reconcileVideoWorkers(const std::vector<Clip> &clips);
    void startAudioWorker(const Clip &clip);
    void startVideoWorker(const Clip &clip);

    void audioMixLoop();
    void videoComposeLoop();
    void pulseLoop();

    int decodeAudioWorker(AudioWorker &worker);
    int decodeVideoWorker(VideoWorker &worker);
    QImage composeDraftFrameAt(qint64 timelineMs, const std::vector<Clip> &videoClips);
    QImage decodeVideoFrameAt(const Clip &clip, qint64 sourceTimeMs);
    static std::string finalViewKey(qint64 timelineMs, const std::vector<std::shared_ptr<VideoWorker>> &workers);
    static std::string audioBlockKey(const std::string &clipWorkerKey, qint64 relativeBlockIndex);
    static std::string videoFrameKey(const QString &sourcePath, qint64 sourceTimeMs, bool draft);
    void markResourceBucket(const QString &sourcePath, qint64 bucketMs);
    void markAudioBucket(const QString &sourcePath, qint64 bucketMs);
    void markFinalTimelineFrame(qint64 timelineMs);

    static std::string clipKey(const Clip &clip);

private:
    std::function<void(QImage &&)> onVideoFrameReady;

    std::atomic_bool running{false};
    std::atomic_int generation{0};
    std::atomic<qint64> timelineMs{0};
    std::atomic<qint64> playbackStartTimelineMs{0};
    std::atomic<qint64> pulseCounterMs{0};
    std::atomic<qint64> consumedAudioBlock{0};
    std::atomic<qint64> consumedVideoFrame{0};
    std::atomic<qint64> lastAudioReconcileMs{-1};
    std::atomic<qint64> lastVideoReconcileMs{-1};

    std::mutex audioMutex;
    std::unordered_map<std::string, std::shared_ptr<AudioWorker>> audioWorkers;
    qint64 nextAudioBlock = 0;
    QIODevice *audioDevice = nullptr;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    QAudioSink *audioSink = nullptr;
#else
    QAudioOutput *audioSink = nullptr;
#endif

    std::mutex videoMutex;
    std::unordered_map<std::string, std::shared_ptr<VideoWorker>> videoWorkers;
    qint64 nextVideoFrame = 0;

    std::thread audioThread;
    std::thread videoThread;
    std::thread pulseThread;

    std::atomic_bool draftPreviewEnabled{true};
    ProjectSettings projectSettings;
    mutable std::mutex settingsMutex;
    FrameCache frameCache;
    FrameCache finalViewCache;
    std::unique_ptr<AudioBlockCache> audioBlockCache;
    std::atomic<double> composeFps{0.0};
    std::atomic<quint64> composedFrames{0};
    std::atomic<quint64> draftFramesDecoded{0};
    std::atomic<quint64> finalViewCacheHits{0};
    std::atomic<quint64> finalViewCacheMisses{0};
    std::atomic<quint64> avcppOpenCalls{0};
    std::atomic<quint64> avcppPacketReads{0};
    std::atomic<quint64> avcppVideoDecodeCalls{0};
    std::atomic<quint64> avcppAudioDecodeCalls{0};
    mutable std::mutex coverageMutex;
    std::unordered_map<std::string, std::unordered_set<qint64>> videoBucketsBySource;
    std::unordered_map<std::string, std::unordered_set<qint64>> audioBucketsBySource;
    std::unordered_set<qint64> finalTimelineFrames;
};

#endif // PLAYBACKPIPELINE_H
