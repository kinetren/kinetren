#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QElapsedTimer>
#include <QTimer>
#include <QtGlobal>
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
    void syncTimelinePlaybackRange(qint64 fromMs, qint64 toMs);
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
    qint64 lastPlaybackSyncMs = -1;
    qint64 lastTimelineFrameIndex = -1;
    QElapsedTimer timelinePlaybackClock;
    std::unique_ptr<PlaybackPipeline> playbackPipeline;
    ProjectSettings projectSettings;
    QTimer performanceTimer;
    quint64 lastAvcppTotalCalls = 0;
    quint64 lastAvcppOpenCalls = 0;
    quint64 lastAvcppPacketReads = 0;
    quint64 lastAvcppVideoDecodeCalls = 0;
    quint64 lastAvcppAudioDecodeCalls = 0;
    qint64 lastPerfSampleMs = 0;

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
