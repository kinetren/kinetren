#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QString>
#include <QTimer>
#include <QtGlobal>
#include <avcpp/av.h>
#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE

namespace av {
class VideoFrame;
}

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow() override;

    void startPlayback();

private:
    struct AudioRingBuffer;
    struct AudioTrackWorker;

    void setupTimelineEditorUi();
    void toggleTimelinePlayback();
    void onTimelineTick();
    void syncTimelinePlayback();
    void stopMediaPlayback();
    void stopAudioPlayback();
    void ensureAudioPipelineStarted(qint64 timelineMs);
    void shutdownAudioPipeline();
    void updateAudioWorkers(qint64 timelineMs);
    void mixAndOutputAudio(qint64 timelineMs);
    QString audioWorkerKey(const QString &path, qint64 clipStartMs) const;
    void startAudioWorker(const QString &path, qint64 clipStartMs, qint64 clipDurationMs, qint64 timelineMs);
    void startVideoPlayback(const QString &path, qint64 clipStartMs, qint64 timelineMs);

    int decodeVideoFile(const std::string &uri, qint64 startOffsetMs, int generation);
    int decodeAudioToRing(const std::string &uri,
                          qint64 startOffsetMs,
                          qint64 baseBlockIndex,
                          int generation,
                          const std::shared_ptr<AudioRingBuffer> &ring,
                          const std::shared_ptr<std::atomic_bool> &stopFlag);
    void paintFrame(av::VideoFrame &&frame);

    class SourceAreaWidget *sourceArea = nullptr;
    class TimelineEditor *timelineEditor = nullptr;
    class QPushButton *timelinePlayPauseButton = nullptr;
    class QLabel *timelineOverlayLabel = nullptr;
    QTimer timelineTimer;
    bool timelinePlaying = false;
    qint64 timelineReturnCursorMs = 0;
    std::atomic_int videoPlaybackGeneration{0};
    std::atomic_int audioPlaybackGeneration{0};
    QString currentVideoPath;
    qint64 currentVideoClipStartMs = -1;
    QString currentAudioPath;
    qint64 currentAudioClipStartMs = -1;
    std::unordered_map<QString, std::shared_ptr<AudioTrackWorker>> audioWorkers;
    std::mutex audioWorkersMutex;
    qint64 nextAudioBlockIndex = 0;
    class QIODevice *audioOutputDevice = nullptr;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    class QAudioSink *audioSink = nullptr;
#else
    class QAudioOutput *audioSink = nullptr;
#endif

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
