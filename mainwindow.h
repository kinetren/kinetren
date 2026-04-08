#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QElapsedTimer>
#include <QTimer>
#include <memory>

#include "projectsettings.h"

QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE

class PlaybackPipeline;
class PerformanceWidget;

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
    void toggleTimelinePause();
    void onTimelineTick();
    void syncTimelinePlayback();
    void stopMediaPlayback();
    void refreshTimelineScroll();
    void renderDraftProject();
    void applyProjectSettings();
    void refreshPerformancePanel();

    class SourceAreaWidget *sourceArea = nullptr;
    class TimelineEditor *timelineEditor = nullptr;
    PerformanceWidget *performanceWidget = nullptr;
    class QToolButton *timelinePauseButton = nullptr;
    class QLabel *timelineOverlayLabel = nullptr;
    class QLabel *timelineGlobalTimeLabel = nullptr;
    class QScrollBar *timelineScrollBar = nullptr;
    class QToolButton *timelineLoopButton = nullptr;
    QTimer timelineTimer;
    bool timelinePlaying = false;
    bool stoppedAtRegionEnd = false;
    qint64 timelinePlayStartMs = 0;
    QElapsedTimer timelinePlaybackClock;
    std::unique_ptr<PlaybackPipeline> playbackPipeline;
    ProjectSettings projectSettings;
    QTimer performanceTimer;

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
