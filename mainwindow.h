#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QTimer>
#include <QtGlobal>
#include <QString>
#include <memory>
#include <QHash>

#include "projectsettings.h"

QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE

class PlaybackPipeline;
class PerformanceWidget;
class QCloseEvent;
class QEvent;
class QDockWidget;
class SourceReplacePanel;
class SourceOffsetPanel;
class MorpherEditWindow;

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
    void schedulePlaybackSync();
    void flushPlaybackSync();
    void schedulePerformancePanelRefresh();
    void flushPerformancePanelRefresh();
    void stopMediaPlayback();
    void refreshTimelineScroll();
    void refreshTimelineVerticalScroll();
    void renderDraftProject();
    void applyProjectSettings();
    void refreshPerformancePanel();
    bool saveProjectToPath(const QString &path, bool silent = false);
    bool loadProjectFromPath(const QString &path, bool silent = false);
    QString defaultAutosaveProjectPath() const;
    void loadLastProjectOnStartup();
    void saveUiState();
    void restoreUiState();
    void openMorpherEditWindow(int morpherEventId);
    void pruneMorpherWindows();
    void refreshWindowTitle();

protected:
    void changeEvent(QEvent *event) override;
    void closeEvent(QCloseEvent *event) override;

    class SourceAreaWidget *sourceArea = nullptr;
    class TimelineEditor *timelineEditor = nullptr;
    PerformanceWidget *performanceWidget = nullptr;
    class QToolButton *timelinePauseButton = nullptr;
    class QLabel *timelineOverlayLabel = nullptr;
    class QLabel *timelineGlobalTimeLabel = nullptr;
    class QScrollBar *timelineScrollBar = nullptr;
    class QScrollBar *timelineScrollBarV = nullptr;
    class QToolButton *timelineLoopButton = nullptr;
    QTimer timelineTimer;
    bool timelinePlaying = false;
    bool stoppedAtRegionEnd = false;
    qint64 lastTimelineFrameIndex = -1;
    std::unique_ptr<PlaybackPipeline> playbackPipeline;
    ProjectSettings projectSettings;
    QTimer performanceTimer;
    QTimer playbackSyncDebounceTimer;
    QTimer performanceRefreshDebounceTimer;
    quint64 lastAvcppTotalCalls = 0;
    quint64 lastAvcppOpenCalls = 0;
    quint64 lastAvcppPacketReads = 0;
    quint64 lastAvcppVideoDecodeCalls = 0;
    quint64 lastAvcppAudioDecodeCalls = 0;
    qint64 lastPerfSampleMs = 0;
    QString currentProjectPath;
    QTimer autosaveTimer;
    bool renderInProgress = false;

    SourceReplacePanel *sourceReplacePanel = nullptr;
    SourceOffsetPanel *sourceOffsetPanel = nullptr;
    QDockWidget *replaceSourceDock = nullptr;
    QDockWidget *sourceOffsetDock = nullptr;
    QHash<int, MorpherEditWindow *> morpherEditWindows;

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
