#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QString>
#include <QTimer>
#include <QtGlobal>
#include <avcpp/av.h>
#include <atomic>
#include <string>

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
    void setupTimelineEditorUi();
    void toggleTimelinePlayback();
    void onTimelineTick();
    void syncTimelinePlayback();
    void stopMediaPlayback();
    void stopAudioPlayback();
    void startVideoPlayback(const QString &path, qint64 clipStartMs, qint64 timelineMs);
    void syncAudioPlayback(qint64 timelineMs);

    int decodeVideoFile(const std::string &uri, qint64 startOffsetMs, int generation);
    void paintFrame(av::VideoFrame &&frame);

    class SourceAreaWidget *sourceArea = nullptr;
    class TimelineEditor *timelineEditor = nullptr;
    class QPushButton *timelinePlayPauseButton = nullptr;
    class QLabel *timelineOverlayLabel = nullptr;
    QTimer timelineTimer;
    bool timelinePlaying = false;
    qint64 timelineReturnCursorMs = 0;
    std::atomic_int playbackGeneration{0};
    QString currentVideoPath;
    qint64 currentVideoClipStartMs = -1;
    class QMediaPlayer *audioPlayer = nullptr;
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    class QAudioOutput *audioOutput = nullptr;
#endif
    QString currentAudioPath;
    qint64 currentAudioClipStartMs = -1;

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
