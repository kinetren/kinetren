#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "performancewidget.h"
#include "playbackpipeline.h"
#include "projectrenderer.h"
#include "previewer.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"

#include <QHBoxLayout>
#include <QCheckBox>
#include <QComboBox>
#include <QLabel>
#include <QMessageBox>
#include <QGuiApplication>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QSaveFile>
#include <QMetaObject>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPushButton>
#include <QShortcut>
#include <QScreen>
#include <QScrollBar>
#include <QSettings>
#include <QSpinBox>
#include <QStandardPaths>
#include <QStyle>
#include <QStatusBar>
#include <QSplitter>
#include <QToolButton>
#include <QVBoxLayout>
#include <QPair>
#include <QPointer>
#include <QVector>
#include <QCloseEvent>

#include <QtGlobal>

#include <algorithm>
#include <climits>
#include <cmath>
#include <thread>
#include <vector>

namespace {
constexpr int kTimelineUiTickMs = 8;
}

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(new Ui::MainWindow)
{
    ui->setupUi(this);
    setupTimelineEditorUi();
    playbackPipeline = std::make_unique<PlaybackPipeline>(
        [this](QImage &&image) {
            auto *frameView = ui->frame;
            QMetaObject::invokeMethod(
                frameView,
                [frameView, image = std::move(image)]() mutable {
                    frameView->paintIt(std::move(image));
                },
                Qt::QueuedConnection);
        });
    applyProjectSettings();
    performanceTimer.setInterval(500);
    connect(&performanceTimer, &QTimer::timeout, this, &MainWindow::refreshPerformancePanel);
    performanceTimer.start();
    syncTimelinePlayback();
    currentProjectPath = defaultAutosaveProjectPath();
    loadLastProjectOnStartup();
    autosaveTimer.setInterval(4000);
    connect(&autosaveTimer, &QTimer::timeout, this, [this]() {
        saveProjectToPath(currentProjectPath, true);
    });
    autosaveTimer.start();
}

MainWindow::~MainWindow()
{
    saveProjectToPath(currentProjectPath, true);
    stopMediaPlayback();
    delete ui;
}

void MainWindow::startPlayback()
{
    syncTimelinePlayback();
}

void MainWindow::setupTimelineEditorUi()
{
    auto *sourceLayout = new QVBoxLayout(ui->widget_3);
    sourceLayout->setContentsMargins(6, 6, 6, 6);
    sourceLayout->setSpacing(6);

    auto *sourceTitle = new QLabel("Source area (drop files here, then drag to timeline)", ui->widget_3);
    sourceLayout->addWidget(sourceTitle);

    sourceArea = new SourceAreaWidget(ui->widget_3);
    sourceArea->setMinimumHeight(110);
    sourceArea->setMaximumHeight(220);
    sourceLayout->addWidget(sourceArea, 1);
    performanceWidget = new PerformanceWidget(ui->widget_3);
    performanceWidget->setMinimumHeight(700);
    sourceLayout->addWidget(performanceWidget, 4);
    sourceLayout->setStretch(0, 0); // title
    sourceLayout->setStretch(1, 1); // source list
    sourceLayout->setStretch(2, 5); // performance panel

    auto *previewHost = new QWidget(ui->splitter);
    auto *previewLayout = new QVBoxLayout(previewHost);
    previewLayout->setContentsMargins(6, 6, 6, 6);
    previewLayout->setSpacing(6);
    const int previewIndex = ui->splitter->indexOf(ui->frame);
    if (previewIndex >= 0) {
        ui->splitter->replaceWidget(previewIndex, previewHost);
        ui->frame->setParent(previewHost);
    }
    previewLayout->addWidget(ui->frame, 1);

    auto *previewControls = new QHBoxLayout();
    auto *previewQualityLabel = new QLabel("Cache quality:", previewHost);
    auto *previewQualityCombo = new QComboBox(previewHost);
    previewQualityCombo->addItem("1/16");
    previewQualityCombo->addItem("1/8");
    previewQualityCombo->addItem("1/4");
    previewQualityCombo->addItem("1/2");
    previewQualityCombo->addItem("1/1 (src)");
    previewQualityCombo->setCurrentIndex(2);
    auto *fpsLabel = new QLabel("FPS:", previewHost);
    auto *fpsCombo = new QComboBox(previewHost);
    fpsCombo->addItem("24");
    fpsCombo->addItem("25");
    fpsCombo->addItem("30");
    fpsCombo->addItem("60");
    fpsCombo->setCurrentText(QString::number(projectSettings.fps));
    auto *seekLookbackLabel = new QLabel("Read step:", previewHost);
    auto *seekLookbackSpin = new QSpinBox(previewHost);
    seekLookbackSpin->setRange(0, 2000);
    seekLookbackSpin->setSingleStep(1);
    seekLookbackSpin->setSuffix(" ms");
    seekLookbackSpin->setSpecialValueText("Auto (1 frame)");
    seekLookbackSpin->setToolTip("How far back we seek before decoding. Auto = 1 frame at current FPS.");
    seekLookbackSpin->setValue(std::max(0, projectSettings.decodeSeekLookbackMs));
    auto *resolutionLabel = new QLabel("Resolution:", previewHost);
    auto *resolutionCombo = new QComboBox(previewHost);
    resolutionCombo->addItem("640x360");
    resolutionCombo->addItem("1280x720");
    resolutionCombo->addItem("1920x1080");
    resolutionCombo->setCurrentText("1280x720");
    auto *smoothScaleCheck = new QCheckBox("Smooth scaling", previewHost);
    smoothScaleCheck->setChecked(false);
    auto *aaCheck = new QCheckBox("Antialiasing", previewHost);
    previewControls->addWidget(previewQualityLabel);
    previewControls->addWidget(previewQualityCombo);
    previewControls->addWidget(fpsLabel);
    previewControls->addWidget(fpsCombo);
    previewControls->addWidget(seekLookbackLabel);
    previewControls->addWidget(seekLookbackSpin);
    previewControls->addWidget(resolutionLabel);
    previewControls->addWidget(resolutionCombo);
    previewControls->addSpacing(10);
    previewControls->addWidget(smoothScaleCheck);
    previewControls->addWidget(aaCheck);
    previewControls->addStretch(1);
    previewLayout->addLayout(previewControls);

    auto *timelineLayout = new QVBoxLayout(ui->widget_2);
    timelineLayout->setContentsMargins(6, 6, 6, 6);
    timelineLayout->setSpacing(6);

    auto *globalCounterWidget = new QWidget(ui->splitter_2);
    globalCounterWidget->setMinimumHeight(28);
    globalCounterWidget->setMaximumHeight(34);
    auto *globalCounterLayout = new QHBoxLayout(globalCounterWidget);
    globalCounterLayout->setContentsMargins(8, 4, 8, 4);
    timelineGlobalTimeLabel = new QLabel("00:00:00.000 F00", globalCounterWidget);
    timelineGlobalTimeLabel->setStyleSheet("font-weight:600; color:#d9e3ef;");
    globalCounterLayout->addWidget(new QLabel("Timeline time:", globalCounterWidget));
    globalCounterLayout->addWidget(timelineGlobalTimeLabel, 1);
    ui->splitter_2->insertWidget(1, globalCounterWidget);
    const int sourcePaneIndex = ui->splitter_2->indexOf(ui->widget_3);
    if (sourcePaneIndex >= 0) {
        ui->splitter_2->setStretchFactor(sourcePaneIndex, 3);
    }
    const int timelinePaneIndex = ui->splitter_2->indexOf(ui->widget_2);
    if (timelinePaneIndex >= 0) {
        ui->splitter_2->setStretchFactor(timelinePaneIndex, 2);
    }

    timelineEditor = new TimelineEditor(ui->widget_2);
    timelineLayout->addWidget(timelineEditor, 1);
    timelineScrollBar = new QScrollBar(Qt::Horizontal, ui->widget_2);
    timelineLayout->addWidget(timelineScrollBar);

    auto *controlsLayout = new QHBoxLayout();
    ui->widget_2->setStyleSheet(
        "QWidget { background-color:#1b1d20; color:#cfd6df; }"
        "QLabel { color:#cfd6df; background-color:transparent; }"
        "QToolButton, QPushButton { background-color:#262a2f; color:#cfd6df; border:1px solid #343a42; border-radius:4px; padding:2px; }"
        "QToolButton:hover, QPushButton:hover { background-color:#313740; border-color:#4b5563; }"
        "QToolButton:pressed, QPushButton:pressed { background-color:#3a4350; }"
        "QToolButton:checked, QPushButton:checked { background-color:#27435e; border-color:#5f89ad; }"
        "QScrollBar:horizontal { background-color:#17191c; height:14px; margin:0; border:1px solid #2a2f35; }"
        "QScrollBar::handle:horizontal { background-color:#3a4350; min-width:28px; border-radius:4px; }"
        "QScrollBar::handle:horizontal:hover { background-color:#4a5666; }"
        "QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal { width:0; background:transparent; }"
        "QScrollBar::add-page:horizontal, QScrollBar::sub-page:horizontal { background-color:#1f2328; }"
        "QToolTip { background-color:#22262b; color:#d7dee8; border:1px solid #3b4350; }");
    timelinePauseButton = new QToolButton(ui->widget_2);
    auto *markerButton = new QToolButton(ui->widget_2);
    auto *addVideoTrackButton = new QToolButton(ui->widget_2);
    auto *addAudioTrackButton = new QToolButton(ui->widget_2);
    auto *splitButton = new QToolButton(ui->widget_2);
    auto *copyButton = new QToolButton(ui->widget_2);
    auto *pasteButton = new QToolButton(ui->widget_2);
    auto *trimStartButton = new QToolButton(ui->widget_2);
    auto *trimEndButton = new QToolButton(ui->widget_2);
    auto *moveTrackUpButton = new QToolButton(ui->widget_2);
    auto *moveTrackDownButton = new QToolButton(ui->widget_2);
    auto *zoomOutButton = new QToolButton(ui->widget_2);
    auto *zoomInButton = new QToolButton(ui->widget_2);
    auto *setRegionStartButton = new QToolButton(ui->widget_2);
    auto *setRegionEndButton = new QToolButton(ui->widget_2);
    auto *clearRegionButton = new QToolButton(ui->widget_2);
    auto *renderDraftButton = new QToolButton(ui->widget_2);
    timelineLoopButton = new QToolButton(ui->widget_2);

    timelinePauseButton->setIcon(style()->standardIcon(QStyle::SP_MediaPlay));
    timelinePauseButton->setToolTip("Play/Pause (Space)");
    markerButton->setIcon(style()->standardIcon(QStyle::SP_DialogYesButton));
    markerButton->setToolTip("Add marker");
    addVideoTrackButton->setIcon(style()->standardIcon(QStyle::SP_FileDialogNewFolder));
    addVideoTrackButton->setToolTip("Add video track");
    addAudioTrackButton->setIcon(style()->standardIcon(QStyle::SP_DirIcon));
    addAudioTrackButton->setToolTip("Add audio track");
    splitButton->setIcon(style()->standardIcon(QStyle::SP_TitleBarShadeButton));
    splitButton->setToolTip("Cut selected event (S)");
    copyButton->setIcon(style()->standardIcon(QStyle::SP_FileDialogContentsView));
    copyButton->setToolTip("Copy selected (Ctrl+C)");
    pasteButton->setIcon(style()->standardIcon(QStyle::SP_FileDialogListView));
    pasteButton->setToolTip("Paste at cursor (Ctrl+V)");
    trimStartButton->setIcon(style()->standardIcon(QStyle::SP_ArrowBack));
    trimStartButton->setToolTip("Trim start");
    trimEndButton->setIcon(style()->standardIcon(QStyle::SP_ArrowForward));
    trimEndButton->setToolTip("Trim end");
    moveTrackUpButton->setIcon(style()->standardIcon(QStyle::SP_ArrowUp));
    moveTrackUpButton->setToolTip("Move selected up");
    moveTrackDownButton->setIcon(style()->standardIcon(QStyle::SP_ArrowDown));
    moveTrackDownButton->setToolTip("Move selected down");
    zoomOutButton->setIcon(style()->standardIcon(QStyle::SP_ArrowLeft));
    zoomOutButton->setToolTip("Zoom out timeline (Ctrl+Wheel)");
    zoomInButton->setIcon(style()->standardIcon(QStyle::SP_ArrowRight));
    zoomInButton->setToolTip("Zoom in timeline (Ctrl+Wheel)");
    setRegionStartButton->setIcon(style()->standardIcon(QStyle::SP_MediaSeekBackward));
    setRegionStartButton->setToolTip("Set region in");
    setRegionEndButton->setIcon(style()->standardIcon(QStyle::SP_MediaSeekForward));
    setRegionEndButton->setToolTip("Set region out");
    clearRegionButton->setIcon(style()->standardIcon(QStyle::SP_DialogResetButton));
    clearRegionButton->setToolTip("Clear region");
    renderDraftButton->setIcon(style()->standardIcon(QStyle::SP_DialogSaveButton));
    renderDraftButton->setToolTip("Render MP4 (video + audio)");
    timelineLoopButton->setIcon(style()->standardIcon(QStyle::SP_BrowserReload));
    timelineLoopButton->setToolTip("Loop region");
    timelineLoopButton->setCheckable(true);

    const QScreen *screenRef = QGuiApplication::primaryScreen();
    const qreal dpiScale = screenRef ? std::clamp(screenRef->logicalDotsPerInch() / 96.0, 1.0, 2.4) : 1.0;
    const int buttonW = qRound(30.0 * dpiScale);
    const int buttonH = qRound(28.0 * dpiScale);
    const int iconPx = qRound(18.0 * dpiScale);

    const auto configureToolButton = [iconPx, buttonW, buttonH](QToolButton *b) {
        b->setIconSize(QSize(iconPx, iconPx));
        b->setAutoRaise(false);
        b->setFixedSize(buttonW, buttonH);
    };
    configureToolButton(timelinePauseButton);
    configureToolButton(markerButton);
    configureToolButton(addVideoTrackButton);
    configureToolButton(addAudioTrackButton);
    configureToolButton(splitButton);
    configureToolButton(copyButton);
    configureToolButton(pasteButton);
    configureToolButton(trimStartButton);
    configureToolButton(trimEndButton);
    configureToolButton(moveTrackUpButton);
    configureToolButton(moveTrackDownButton);
    configureToolButton(zoomOutButton);
    configureToolButton(zoomInButton);
    configureToolButton(setRegionStartButton);
    configureToolButton(setRegionEndButton);
    configureToolButton(clearRegionButton);
    configureToolButton(renderDraftButton);
    configureToolButton(timelineLoopButton);

    timelineOverlayLabel = new QLabel("Overlay @ 0.00s | Video layers: 0 | Audio layers: 0", ui->widget_2);
    controlsLayout->addWidget(timelinePauseButton);
    controlsLayout->addWidget(markerButton);
    controlsLayout->addWidget(addVideoTrackButton);
    controlsLayout->addWidget(addAudioTrackButton);
    controlsLayout->addWidget(splitButton);
    controlsLayout->addWidget(copyButton);
    controlsLayout->addWidget(pasteButton);
    controlsLayout->addWidget(trimStartButton);
    controlsLayout->addWidget(trimEndButton);
    controlsLayout->addWidget(moveTrackUpButton);
    controlsLayout->addWidget(moveTrackDownButton);
    controlsLayout->addWidget(zoomOutButton);
    controlsLayout->addWidget(zoomInButton);
    controlsLayout->addWidget(setRegionStartButton);
    controlsLayout->addWidget(setRegionEndButton);
    controlsLayout->addWidget(clearRegionButton);
    controlsLayout->addWidget(renderDraftButton);
    controlsLayout->addWidget(timelineLoopButton);
    controlsLayout->addWidget(timelineOverlayLabel, 1);
    timelineLayout->addLayout(controlsLayout);

    timelineTimer.setInterval(kTimelineUiTickMs);
    timelineTimer.setTimerType(Qt::PreciseTimer);
    connect(&timelineTimer, &QTimer::timeout, this, &MainWindow::onTimelineTick);
    connect(timelinePauseButton, &QToolButton::clicked, this, &MainWindow::toggleTimelinePause);
    connect(markerButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addMarkerAtCursor);
    connect(addVideoTrackButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addVideoTrack);
    connect(addAudioTrackButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addAudioTrack);
    connect(splitButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::splitSelectedClipAtCursor);
    connect(trimStartButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::trimSelectedClipStartToCursor);
    connect(trimEndButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::trimSelectedClipEndToCursor);
    connect(moveTrackUpButton, &QToolButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->moveSelectedClipToAdjacentTrack(-1);
        }
    });
    connect(moveTrackDownButton, &QToolButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->moveSelectedClipToAdjacentTrack(1);
        }
    });
    connect(zoomOutButton, &QToolButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->setZoomFactor(timelineEditor->zoomFactor() / 1.2);
        }
    });
    connect(zoomInButton, &QToolButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->setZoomFactor(timelineEditor->zoomFactor() * 1.2);
        }
    });
    connect(setRegionStartButton, &QToolButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->setSelectionRegionFromCursor(true);
        }
    });
    connect(setRegionEndButton, &QToolButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->setSelectionRegionFromCursor(false);
        }
    });
    connect(clearRegionButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::clearSelectionRegion);
    connect(renderDraftButton, &QToolButton::clicked, this, &MainWindow::renderDraftProject);
    connect(copyButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::copySelectedToClipboard);
    connect(pasteButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::pasteClipboardAtCursor);
    connect(timelineLoopButton, &QToolButton::toggled, timelineEditor, &TimelineEditor::setLoopEnabled);
    connect(timelineEditor, &TimelineEditor::loopEnabledChanged, timelineLoopButton, &QToolButton::setChecked);
    connect(timelineEditor, &TimelineEditor::overlayInfoChanged, timelineOverlayLabel, &QLabel::setText);
    connect(timelineEditor, &TimelineEditor::cursorChanged, this, [this](qint64 t) {
        if (!timelinePlaying) {
            refreshTimelineScroll();
            if (timelineGlobalTimeLabel && timelineEditor) {
                timelineGlobalTimeLabel->setText(timelineEditor->formatTimelineTime(t));
            }
            syncTimelinePlayback();
        }
    });
    connect(timelineEditor, &TimelineEditor::timelineRangeChanged, this, [this](qint64, qint64) {
        refreshTimelineScroll();
        if (!timelinePlaying) {
            syncTimelinePlayback();
        }
    });
    connect(timelineEditor, &TimelineEditor::viewStartChanged, this, [this](qint64, qint64) {
        refreshTimelineScroll();
    });
    connect(timelineEditor, &TimelineEditor::timelineContentChanged, this, [this]() {
        if (!playbackPipeline) {
            return;
        }
        playbackPipeline->clearTimelineCoverage();
        syncTimelinePlayback();
        refreshPerformancePanel();
    });
    connect(timelineScrollBar, &QScrollBar::valueChanged, [this](int value) {
        if (timelineEditor) {
            timelineEditor->setViewStartMs(static_cast<qint64>(value));
        }
    });
    connect(previewQualityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int index) {
        switch (index) {
        case 0:
            projectSettings.cacheQuality = ProjectSettings::CacheQuality::Quarter16;
            break;
        case 1:
            projectSettings.cacheQuality = ProjectSettings::CacheQuality::Quarter8;
            break;
        case 2:
            projectSettings.cacheQuality = ProjectSettings::CacheQuality::Quarter4;
            break;
        case 3:
            projectSettings.cacheQuality = ProjectSettings::CacheQuality::Half;
            break;
        case 4:
        default:
            projectSettings.cacheQuality = ProjectSettings::CacheQuality::Source;
            break;
        }
        applyProjectSettings();
        if (!timelinePlaying) {
            syncTimelinePlayback();
        }
    });
    connect(fpsCombo, &QComboBox::currentTextChanged, this, [this](const QString &text) {
        bool ok = false;
        const int fps = text.toInt(&ok);
        if (!ok || fps <= 0) {
            return;
        }
        projectSettings.fps = fps;
        applyProjectSettings();
    });
    connect(seekLookbackSpin, QOverload<int>::of(&QSpinBox::valueChanged), this, [this](int value) {
        projectSettings.decodeSeekLookbackMs = std::max(0, value);
        applyProjectSettings();
    });
    connect(resolutionCombo, &QComboBox::currentTextChanged, this, [this](const QString &text) {
        const QStringList parts = text.split('x');
        if (parts.size() != 2) {
            return;
        }
        bool okW = false;
        bool okH = false;
        const int w = parts[0].toInt(&okW);
        const int h = parts[1].toInt(&okH);
        if (!okW || !okH || w <= 0 || h <= 0) {
            return;
        }
        projectSettings.resolution = QSize(w, h);
        applyProjectSettings();
    });
    connect(smoothScaleCheck, &QCheckBox::toggled, ui->frame, &Previewer::setSmoothScaling);
    connect(aaCheck, &QCheckBox::toggled, ui->frame, &Previewer::setAntialiasing);

    auto *spaceShortcut = new QShortcut(QKeySequence(Qt::Key_Space), this);
    connect(spaceShortcut, &QShortcut::activated, this, &MainWindow::toggleTimelinePause);
    auto *splitShortcut = new QShortcut(QKeySequence(Qt::Key_S), this);
    connect(splitShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->splitSelectedClipAtCursor();
        }
    });
    auto *copyShortcut = new QShortcut(QKeySequence::Copy, this);
    connect(copyShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->copySelectedToClipboard();
        }
    });
    auto *pasteShortcut = new QShortcut(QKeySequence::Paste, this);
    connect(pasteShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->pasteClipboardAtCursor();
        }
    });
    auto *deleteShortcut = new QShortcut(QKeySequence::Delete, this);
    connect(deleteShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->deleteSelectedClip();
        }
    });

    const auto applyStatusTip = [](QToolButton *button) {
        button->setStatusTip(button->toolTip());
    };
    applyStatusTip(timelinePauseButton);
    applyStatusTip(markerButton);
    applyStatusTip(addVideoTrackButton);
    applyStatusTip(addAudioTrackButton);
    applyStatusTip(splitButton);
    applyStatusTip(copyButton);
    applyStatusTip(pasteButton);
    applyStatusTip(trimStartButton);
    applyStatusTip(trimEndButton);
    applyStatusTip(moveTrackUpButton);
    applyStatusTip(moveTrackDownButton);
    applyStatusTip(zoomOutButton);
    applyStatusTip(zoomInButton);
    applyStatusTip(setRegionStartButton);
    applyStatusTip(setRegionEndButton);
    applyStatusTip(clearRegionButton);
    applyStatusTip(renderDraftButton);
    applyStatusTip(timelineLoopButton);
    timelineEditor->setStatusTip("Timeline: drag events, wheel to scroll, Ctrl+wheel to zoom");
    timelineScrollBar->setStatusTip("Timeline scrollbar");
    if (timelineGlobalTimeLabel && timelineEditor) {
        timelineGlobalTimeLabel->setText(timelineEditor->formatTimelineTime(timelineEditor->cursorMs()));
    }
    statusBar()->showMessage("Ready. Hover controls to see action hints.", 2500);

    refreshTimelineScroll();
    refreshPerformancePanel();
}

void MainWindow::toggleTimelinePlayback()
{
    toggleTimelinePause();
}

void MainWindow::toggleTimelinePause()
{
    if (!timelineEditor) {
        return;
    }

    if (!timelinePlaying) {
        if (stoppedAtRegionEnd && timelineEditor->hasSelectionRegion() &&
            timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
            timelineEditor->setCursorMs(timelineEditor->selectionRegionStartMs(), true);
        }
        timelinePlaying = true;
        stoppedAtRegionEnd = false;
        timelinePauseButton->setIcon(style()->standardIcon(QStyle::SP_MediaPause));
        lastTimelineFrameIndex = -1;
        if (playbackPipeline) {
            if (timelineEditor->hasSelectionRegion() &&
                timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
                const auto mode = timelineEditor->loopEnabled()
                    ? burstcon::PlaybackMode::RegionLoop
                    : burstcon::PlaybackMode::Region;
                playbackPipeline->setPlaybackMode(
                    mode,
                    timelineEditor->selectionRegionStartMs(),
                    timelineEditor->selectionRegionEndMs());
            } else {
                playbackPipeline->setPlaybackMode(burstcon::PlaybackMode::Free, 0, 0);
            }
            playbackPipeline->start(timelineEditor->cursorMs());
        }
        syncTimelinePlayback();
        timelineTimer.start();
        return;
    }

    timelinePlaying = false;
    stoppedAtRegionEnd = false;
    lastTimelineFrameIndex = -1;
    timelineTimer.stop();
    stopMediaPlayback();
    if (timelineEditor->hasSelectionRegion() &&
        timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
        timelineEditor->setCursorMs(timelineEditor->selectionRegionStartMs(), true);
    }
    timelinePauseButton->setIcon(style()->standardIcon(QStyle::SP_MediaPlay));
}

void MainWindow::onTimelineTick()
{
    if (!timelineEditor || !timelinePlaying) {
        return;
    }

    qint64 playbackMs = timelineEditor->cursorMs();
    if (playbackPipeline) {
        playbackMs = playbackPipeline->playbackPositionMs();
        if (!playbackPipeline->isRunning()) {
            timelinePlaying = false;
            stoppedAtRegionEnd = timelineEditor->hasSelectionRegion() && !timelineEditor->loopEnabled();
            timelineTimer.stop();
            stopMediaPlayback();
            timelinePauseButton->setIcon(style()->standardIcon(QStyle::SP_MediaPlay));
        }
    }

    // Keep playhead moving but let users freely navigate the timeline view while playing.
    timelineEditor->setCursorMs(playbackMs, false);
    if (timelineGlobalTimeLabel) {
        timelineGlobalTimeLabel->setText(timelineEditor->formatTimelineTime(playbackMs));
    }

    syncTimelinePlayback();
}

void MainWindow::syncTimelinePlayback()
{
    if (!timelineEditor || !playbackPipeline) {
        return;
    }
    const qint64 tMs = timelineEditor->cursorMs();
    const auto clips = timelineEditor->renderClips();

    std::vector<PlaybackPipeline::Clip> vclips;
    std::vector<PlaybackPipeline::Clip> aclips;
    vclips.reserve(clips.size());
    aclips.reserve(clips.size());
    for (const auto &c : clips) {
        const PlaybackPipeline::Clip clip{c.sourcePath, c.startMs, c.durationMs, c.sourceInMs};
        if (c.isVideo) {
            vclips.push_back(clip);
        }
        if (c.isAudio) {
            aclips.push_back(clip);
        }
    }

    playbackPipeline->update(tMs, vclips, aclips);
    if (timelineEditor->hasSelectionRegion() &&
        timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
        const auto mode = timelineEditor->loopEnabled()
            ? burstcon::PlaybackMode::RegionLoop
            : burstcon::PlaybackMode::Region;
        playbackPipeline->setPlaybackMode(
            mode,
            timelineEditor->selectionRegionStartMs(),
            timelineEditor->selectionRegionEndMs());
    } else {
        playbackPipeline->setPlaybackMode(burstcon::PlaybackMode::Free, 0, 0);
    }
}

void MainWindow::refreshTimelineScroll()
{
    if (!timelineEditor || !timelineScrollBar) {
        return;
    }
    const qint64 maxView = timelineEditor->maxViewStartMs();
    const qint64 visible = timelineEditor->visibleDurationMs();
    const qint64 page = std::max<qint64>(1, visible / 10);
    timelineScrollBar->blockSignals(true);
    timelineScrollBar->setRange(0, static_cast<int>(std::min<qint64>(INT_MAX, maxView)));
    timelineScrollBar->setPageStep(static_cast<int>(std::min<qint64>(INT_MAX, page)));
    timelineScrollBar->setValue(static_cast<int>(std::min<qint64>(INT_MAX, timelineEditor->viewStartMs())));
    timelineScrollBar->blockSignals(false);
}

void MainWindow::stopMediaPlayback()
{
    if (playbackPipeline) {
        playbackPipeline->stop();
    }
}

void MainWindow::renderDraftProject()
{
    if (!timelineEditor || !playbackPipeline) {
        return;
    }
    if (renderInProgress) {
        statusBar()->showMessage("Render already in progress.", 3000);
        return;
    }

    const qint64 startMs = (timelineEditor->hasSelectionRegion() && timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs())
        ? timelineEditor->selectionRegionStartMs()
        : 0;
    const qint64 endMs = (timelineEditor->hasSelectionRegion() && timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs())
        ? timelineEditor->selectionRegionEndMs()
        : timelineEditor->maxTimelineMs();
    if (endMs <= startMs) {
        statusBar()->showMessage("Draft render skipped: empty range.", 3500);
        return;
    }

    const QString defaultOut = QDir::homePath() + "/kp-render-" +
        QDateTime::currentDateTime().toString("yyyyMMdd-hhmmss") + ".mp4";
    QFileDialog saveDialog(this, "Render MP4", defaultOut, "MP4 Video (*.mp4)");
    saveDialog.setAcceptMode(QFileDialog::AcceptSave);
    saveDialog.setFileMode(QFileDialog::AnyFile);
    saveDialog.setDefaultSuffix("mp4");
    saveDialog.setOption(QFileDialog::DontUseNativeDialog, true);
    saveDialog.selectFile(defaultOut);
    if (saveDialog.exec() != QDialog::Accepted) {
        return;
    }
    const QStringList selectedFiles = saveDialog.selectedFiles();
    const QString outPath = selectedFiles.isEmpty() ? QString() : selectedFiles.first();
    if (outPath.isEmpty()) {
        return;
    }

    statusBar()->showMessage("Rendering MP4 (video + audio)...", 0);
    renderInProgress = true;
    const auto clips = timelineEditor->renderClips();
    const ProjectSettings settings = projectSettings;
    QPointer<MainWindow> guard(this);
    std::thread([guard, clips, settings, startMs, endMs, outPath]() {
        const auto result = ProjectRenderer::renderMp4(clips, settings, startMs, endMs, outPath);
        if (!guard) {
            return;
        }
        QMetaObject::invokeMethod(
            guard.data(),
            [guard, result]() {
                if (!guard) {
                    return;
                }
                guard->renderInProgress = false;
                if (!result.ok) {
                    QString brief = "Render failed: " + result.message;
                    if (!result.logPath.isEmpty()) {
                        brief += " (log: " + result.logPath + ")";
                    }
                    guard->statusBar()->showMessage(brief, 20000);
                    QMessageBox msg(guard.data());
                    msg.setIcon(QMessageBox::Critical);
                    msg.setWindowTitle("Render Failed");
                    msg.setText(result.message);
                    QString details = result.details;
                    if (!result.logPath.isEmpty()) {
                        details += "\n\nSaved log: " + result.logPath;
                    }
                    if (details.isEmpty()) {
                        details = "No additional details available.";
                    }
                    msg.setDetailedText(details);
                    msg.exec();
                    return;
                }
                guard->statusBar()->showMessage(result.message, 10000);
            },
            Qt::QueuedConnection);
    }).detach();
}

void MainWindow::applyProjectSettings()
{
    if (!playbackPipeline) {
        return;
    }
    if (timelineEditor) {
        timelineEditor->setTimelineFps(projectSettings.fps);
    }
    timelineTimer.setInterval(kTimelineUiTickMs);
    lastTimelineFrameIndex = -1;
    playbackPipeline->setProjectSettings(projectSettings);
    playbackPipeline->setDraftPreviewEnabled(projectSettings.cacheQuality != ProjectSettings::CacheQuality::Source);
    refreshPerformancePanel();
}

void MainWindow::refreshPerformancePanel()
{
    if (!playbackPipeline || !performanceWidget || !timelineEditor) {
        return;
    }
    const auto stats = playbackPipeline->performanceSnapshot();
    const quint64 resourceLookups = stats.resourceCacheHits + stats.resourceCacheMisses;
    const double resourceHitPct = (resourceLookups > 0)
        ? (static_cast<double>(stats.resourceCacheHits) * 100.0 / static_cast<double>(resourceLookups))
        : 0.0;
    const quint64 finalLookups = stats.finalViewCacheHits + stats.finalViewCacheMisses;
    const double finalHitPct = (finalLookups > 0)
        ? (static_cast<double>(stats.finalViewCacheHits) * 100.0 / static_cast<double>(finalLookups))
        : 0.0;
    const quint64 audioLookups = stats.audioCacheHits + stats.audioCacheMisses;
    const double audioHitPct = (audioLookups > 0)
        ? (static_cast<double>(stats.audioCacheHits) * 100.0 / static_cast<double>(audioLookups))
        : 0.0;
    const quint64 avcppTotalCalls =
        stats.avcppOpenCalls + stats.avcppPacketReads + stats.avcppVideoDecodeCalls + stats.avcppAudioDecodeCalls;
    const qint64 nowMs = QDateTime::currentMSecsSinceEpoch();
    double avcppCallsPerSec = 0.0;
    double avcppOpenPerSec = 0.0;
    double avcppReadPerSec = 0.0;
    double avcppVideoDecodePerSec = 0.0;
    double avcppAudioDecodePerSec = 0.0;
    if (lastPerfSampleMs > 0 && nowMs > lastPerfSampleMs && avcppTotalCalls >= lastAvcppTotalCalls) {
        const qint64 deltaMs = nowMs - lastPerfSampleMs;
        const quint64 deltaCalls = avcppTotalCalls - lastAvcppTotalCalls;
        avcppCallsPerSec = (deltaMs > 0) ? (static_cast<double>(deltaCalls) * 1000.0 / static_cast<double>(deltaMs)) : 0.0;
        const auto perSec = [deltaMs](quint64 deltaValue) {
            return (deltaMs > 0) ? (static_cast<double>(deltaValue) * 1000.0 / static_cast<double>(deltaMs)) : 0.0;
        };
        avcppOpenPerSec = perSec((stats.avcppOpenCalls >= lastAvcppOpenCalls) ? (stats.avcppOpenCalls - lastAvcppOpenCalls) : 0);
        avcppReadPerSec = perSec((stats.avcppPacketReads >= lastAvcppPacketReads) ? (stats.avcppPacketReads - lastAvcppPacketReads) : 0);
        avcppVideoDecodePerSec =
            perSec((stats.avcppVideoDecodeCalls >= lastAvcppVideoDecodeCalls) ? (stats.avcppVideoDecodeCalls - lastAvcppVideoDecodeCalls) : 0);
        avcppAudioDecodePerSec =
            perSec((stats.avcppAudioDecodeCalls >= lastAvcppAudioDecodeCalls) ? (stats.avcppAudioDecodeCalls - lastAvcppAudioDecodeCalls) : 0);
    }
    lastPerfSampleMs = nowMs;
    lastAvcppTotalCalls = avcppTotalCalls;
    lastAvcppOpenCalls = stats.avcppOpenCalls;
    lastAvcppPacketReads = stats.avcppPacketReads;
    lastAvcppVideoDecodeCalls = stats.avcppVideoDecodeCalls;
    lastAvcppAudioDecodeCalls = stats.avcppAudioDecodeCalls;
    const double aggregateHitPct = (resourceHitPct * 0.45) + (finalHitPct * 0.35) + (audioHitPct * 0.20);

    QHash<QString, QVector<QPair<double, double>>> clipCoverage;
    std::vector<PlaybackPipeline::Clip> videoRenderClips;
    std::vector<PlaybackPipeline::Clip> audioRenderClips;
    for (const auto &clip : timelineEditor->renderClips()) {
        PlaybackPipeline::Clip c{clip.sourcePath, clip.startMs, clip.durationMs, clip.sourceInMs};
        if (clip.isVideo) {
            videoRenderClips.push_back(c);
        }
        if (clip.isAudio) {
            audioRenderClips.push_back(c);
        }
    }
    const auto videoCoverage = playbackPipeline->videoCacheSegments(videoRenderClips);
    const auto audioCoverage = playbackPipeline->audioCacheSegments(audioRenderClips);
    for (auto it = videoCoverage.constBegin(); it != videoCoverage.constEnd(); ++it) {
        clipCoverage.insert(it.key(), it.value());
    }
    for (auto it = audioCoverage.constBegin(); it != audioCoverage.constEnd(); ++it) {
        QVector<QPair<double, double>> merged = clipCoverage.value(it.key());
        merged += it.value();
        std::sort(merged.begin(), merged.end(), [](const QPair<double, double> &a, const QPair<double, double> &b) {
            return a.first < b.first;
        });
        clipCoverage.insert(it.key(), merged);
    }
    const qint64 viewStart = timelineEditor->viewStartMs();
    const qint64 viewEnd = viewStart + timelineEditor->visibleDurationMs();
    const auto timelineSegments = playbackPipeline->finalTimelineSegments(viewStart, viewEnd);
    timelineEditor->setCacheCoverage(clipCoverage, timelineSegments);

    const auto mb = [](quint64 bytes) -> QString {
        return QString::number(static_cast<double>(bytes) / (1024.0 * 1024.0), 'f', 1) + "MB";
    };
    performanceWidget->setMetrics(
        QString("Compose FPS: %1 | Frames: %2")
            .arg(stats.composeFps, 0, 'f', 1)
            .arg(stats.composedFrames),
        QString("Resource cache: %1/%2 (%3%) | Final view: %4/%5 (%6%) | Audio: %7/%8 (%9%)")
            .arg(stats.resourceCacheHits)
            .arg(resourceLookups)
            .arg(resourceHitPct, 0, 'f', 1)
            .arg(stats.finalViewCacheHits)
            .arg(finalLookups)
            .arg(finalHitPct, 0, 'f', 1)
            .arg(stats.audioCacheHits)
            .arg(audioLookups)
            .arg(audioHitPct, 0, 'f', 1),
        QString("Resource cache %1x%2 (%3) | decoded: %4 | entries: %5 | evictions: %6 | used/limit: %7 / %8 (max %9)")
            .arg(projectSettings.cacheResolution().width())
            .arg(projectSettings.cacheResolution().height())
            .arg(projectSettings.cacheQualityLabel())
            .arg(stats.draftFramesDecoded)
            .arg(stats.resourceCacheEntries)
            .arg(stats.resourceCacheEvictions)
            .arg(mb(stats.resourceCacheUsedBytes))
            .arg(mb(stats.resourceCacheLimitBytes))
            .arg(mb(stats.resourceCacheConfiguredMaxBytes)),
        QString("Project: %1 FPS | %2x%3 | avcpp open:%4 read:%5 vdec:%6 adec:%7 | audio-cache entries:%8")
            .arg(projectSettings.fps)
            .arg(projectSettings.resolution.width())
            .arg(projectSettings.resolution.height())
            .arg(stats.avcppOpenCalls)
            .arg(stats.avcppPacketReads)
            .arg(stats.avcppVideoDecodeCalls)
            .arg(stats.avcppAudioDecodeCalls)
            .arg(stats.audioCacheEntries),
        stats.composeFps,
        aggregateHitPct,
        finalHitPct,
        audioHitPct,
        avcppCallsPerSec,
        avcppOpenPerSec,
        avcppReadPerSec,
        avcppVideoDecodePerSec,
        avcppAudioDecodePerSec);
}

bool MainWindow::saveProjectToPath(const QString &path, bool silent)
{
    if (!timelineEditor || !sourceArea) {
        return false;
    }
    if (path.isEmpty()) {
        return false;
    }

    QFileInfo info(path);
    QDir parent = info.dir();
    if (!parent.exists()) {
        parent.mkpath(".");
    }

    QJsonObject settingsObject;
    settingsObject.insert("fps", projectSettings.fps);
    settingsObject.insert("resolutionWidth", projectSettings.resolution.width());
    settingsObject.insert("resolutionHeight", projectSettings.resolution.height());
    settingsObject.insert("cacheQuality", static_cast<int>(projectSettings.cacheQuality));
    settingsObject.insert("decodeSeekLookbackMs", projectSettings.decodeSeekLookbackMs);

    QJsonArray sourcePathsArray;
    const QStringList sourcePaths = sourceArea->sourcePaths();
    for (const QString &sourcePath : sourcePaths) {
        sourcePathsArray.push_back(sourcePath);
    }

    QJsonObject root;
    root.insert("version", 1);
    root.insert("savedAtUtc", QDateTime::currentDateTimeUtc().toString(Qt::ISODate));
    root.insert("projectSettings", settingsObject);
    root.insert("sourcePaths", sourcePathsArray);
    root.insert("timeline", timelineEditor->toJson());

    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        if (!silent) {
            statusBar()->showMessage("Autosave failed: can't open project file.", 6000);
        }
        return false;
    }
    file.write(QJsonDocument(root).toJson(QJsonDocument::Indented));
    if (!file.commit()) {
        if (!silent) {
            statusBar()->showMessage("Autosave failed: can't commit project file.", 6000);
        }
        return false;
    }

    currentProjectPath = path;
    QSettings settings;
    settings.setValue("project/lastPath", currentProjectPath);
    if (!silent) {
        statusBar()->showMessage("Project saved: " + currentProjectPath, 3000);
    }
    return true;
}

bool MainWindow::loadProjectFromPath(const QString &path, bool silent)
{
    if (path.isEmpty() || !timelineEditor || !sourceArea) {
        return false;
    }

    QFile file(path);
    if (!file.exists() || !file.open(QIODevice::ReadOnly)) {
        return false;
    }

    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    if (!doc.isObject()) {
        if (!silent) {
            statusBar()->showMessage("Project load failed: invalid project format.", 6000);
        }
        return false;
    }

    const QJsonObject root = doc.object();
    const QJsonObject settingsObject = root.value("projectSettings").toObject();
    if (!settingsObject.isEmpty()) {
        const int fps = settingsObject.value("fps").toInt(projectSettings.fps);
        const int width = settingsObject.value("resolutionWidth").toInt(projectSettings.resolution.width());
        const int height = settingsObject.value("resolutionHeight").toInt(projectSettings.resolution.height());
        const int cacheQuality = settingsObject.value("cacheQuality").toInt(static_cast<int>(projectSettings.cacheQuality));
        const int decodeSeekLookbackMs = settingsObject.value("decodeSeekLookbackMs").toInt(projectSettings.decodeSeekLookbackMs);

        projectSettings.fps = std::max(1, fps);
        projectSettings.resolution = QSize(std::max(1, width), std::max(1, height));
        projectSettings.cacheQuality = static_cast<ProjectSettings::CacheQuality>(
            std::clamp(cacheQuality,
                       static_cast<int>(ProjectSettings::CacheQuality::Quarter16),
                       static_cast<int>(ProjectSettings::CacheQuality::Source)));
        projectSettings.decodeSeekLookbackMs = std::max(0, decodeSeekLookbackMs);
    }

    QStringList loadedSourcePaths;
    const QJsonArray sourcePathsArray = root.value("sourcePaths").toArray();
    for (const auto &value : sourcePathsArray) {
        const QString pathValue = value.toString();
        if (!pathValue.isEmpty()) {
            loadedSourcePaths.push_back(pathValue);
        }
    }
    sourceArea->setSourcePaths(loadedSourcePaths);

    const QJsonObject timelineObject = root.value("timeline").toObject();
    if (!timelineObject.isEmpty()) {
        timelineEditor->fromJson(timelineObject);
    }
    applyProjectSettings();
    refreshTimelineScroll();
    syncTimelinePlayback();
    refreshPerformancePanel();

    currentProjectPath = path;
    QSettings settings;
    settings.setValue("project/lastPath", currentProjectPath);
    if (!silent) {
        statusBar()->showMessage("Project loaded: " + currentProjectPath, 3000);
    }
    return true;
}

QString MainWindow::defaultAutosaveProjectPath() const
{
    QString appDataDir = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    if (appDataDir.isEmpty()) {
        appDataDir = QDir::homePath() + "/.kp-test";
    }
    QDir dir(appDataDir);
    if (!dir.exists()) {
        dir.mkpath(".");
    }
    return dir.filePath("autosave-project.kpproj.json");
}

void MainWindow::loadLastProjectOnStartup()
{
    QSettings settings;
    const QString settingsPath = settings.value("project/lastPath").toString().trimmed();
    const QString fallbackPath = defaultAutosaveProjectPath();
    if (!settingsPath.isEmpty() && QFileInfo::exists(settingsPath)) {
        if (loadProjectFromPath(settingsPath, true)) {
            statusBar()->showMessage("Loaded last project: " + settingsPath, 4000);
            return;
        }
    }
    if (QFileInfo::exists(fallbackPath)) {
        if (loadProjectFromPath(fallbackPath, true)) {
            statusBar()->showMessage("Loaded autosaved project.", 4000);
            return;
        }
    }
    currentProjectPath = fallbackPath;
}

void MainWindow::closeEvent(QCloseEvent *event)
{
    saveProjectToPath(currentProjectPath, true);
    QMainWindow::closeEvent(event);
}

