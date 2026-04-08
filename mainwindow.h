#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QElapsedTimer>
#include <QTimer>
#include <memory>

QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE

class PlaybackPipeline;

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
    void refreshTimelineScroll();

    class SourceAreaWidget *sourceArea = nullptr;
    class TimelineEditor *timelineEditor = nullptr;
    class QPushButton *timelinePlayPauseButton = nullptr;
    class QLabel *timelineOverlayLabel = nullptr;
    class QScrollBar *timelineScrollBar = nullptr;
    class QCheckBox *timelineLoopCheckBox = nullptr;
    QTimer timelineTimer;
    bool timelinePlaying = false;
    qint64 timelineReturnCursorMs = 0;
    qint64 timelinePlayStartMs = 0;
    QElapsedTimer timelinePlaybackClock;
    std::unique_ptr<PlaybackPipeline> playbackPipeline;

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
