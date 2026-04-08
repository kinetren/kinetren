#ifndef PLAYBACKPIPELINE_H
#define PLAYBACKPIPELINE_H

#include <QImage>
#include <QString>
#include <QtGlobal>

#include <atomic>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
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
    };

    explicit PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady);
    ~PlaybackPipeline();

    void start(qint64 timelineMs);
    void stop();
    void update(qint64 timelineMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips);

private:
    struct AudioRingBuffer;
    struct VideoRingBuffer;
    struct AudioWorker;
    struct VideoWorker;

    void ensureAudioOutput();
    void shutdownAudioOutput();

    void reconcileAudioWorkers(const std::vector<Clip> &clips);
    void reconcileVideoWorkers(const std::vector<Clip> &clips);
    void startAudioWorker(const Clip &clip, qint64 timelineMs);
    void startVideoWorker(const Clip &clip, qint64 timelineMs);

    void audioMixLoop();
    void videoComposeLoop();

    int decodeAudioWorker(const AudioWorker &worker);
    int decodeVideoWorker(const VideoWorker &worker);

    static std::string clipKey(const Clip &clip);

private:
    std::function<void(QImage &&)> onVideoFrameReady;

    std::atomic_bool running{false};
    std::atomic_int generation{0};
    std::atomic<qint64> timelineMs{0};
    std::atomic<qint64> consumedAudioBlock{0};
    std::atomic<qint64> consumedVideoFrame{0};

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
};

#endif // PLAYBACKPIPELINE_H
