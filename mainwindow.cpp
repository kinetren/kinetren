#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "playbackpipeline.h"
#include "previewer.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"

#include <QHBoxLayout>
#include <QCheckBox>
#include <QComboBox>
#include <QLabel>
#include <QGuiApplication>
#include <QDateTime>
#include <QDir>
#include <QMetaObject>
#include <QPushButton>
#include <QShortcut>
#include <QScreen>
#include <QScrollBar>
#include <QStyle>
#include <QStatusBar>
#include <QSplitter>
#include <QToolButton>
#include <QVBoxLayout>

#include <QtGlobal>

#include <algorithm>
#include <climits>
#include <vector>

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
    playbackPipeline->setDraftPreviewEnabled(true);
    syncTimelinePlayback();
}

MainWindow::~MainWindow()
{
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
    sourceLayout->addWidget(sourceArea, 1);

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
    auto *previewQualityLabel = new QLabel("Preview:", previewHost);
    auto *previewQualityCombo = new QComboBox(previewHost);
    previewQualityCombo->addItem("Draft (cached)");
    previewQualityCombo->addItem("Full source");
    auto *smoothScaleCheck = new QCheckBox("Smooth scaling", previewHost);
    smoothScaleCheck->setChecked(true);
    auto *aaCheck = new QCheckBox("Antialiasing", previewHost);
    previewControls->addWidget(previewQualityLabel);
    previewControls->addWidget(previewQualityCombo);
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
    renderDraftButton->setToolTip("Render draft image sequence");
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

    timelineTimer.setInterval(8);
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
        refreshTimelineScroll();
        if (timelineGlobalTimeLabel && timelineEditor) {
            timelineGlobalTimeLabel->setText(timelineEditor->formatTimelineTime(t));
        }
        if (!timelinePlaying) {
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
    connect(timelineScrollBar, &QScrollBar::valueChanged, [this](int value) {
        if (timelineEditor) {
            timelineEditor->setViewStartMs(static_cast<qint64>(value));
        }
    });
    connect(previewQualityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int index) {
        if (playbackPipeline) {
            playbackPipeline->setDraftPreviewEnabled(index == 0);
        }
        if (!timelinePlaying) {
            syncTimelinePlayback();
        }
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
        timelinePlayStartMs = timelineEditor->cursorMs();
        timelinePlaybackClock.restart();
        if (playbackPipeline) {
            playbackPipeline->start(timelineEditor->cursorMs());
        }
        syncTimelinePlayback();
        timelineTimer.start();
        return;
    }

    timelinePlaying = false;
    stoppedAtRegionEnd = false;
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
    qint64 playbackMs = timelinePlayStartMs + timelinePlaybackClock.elapsed();
    if (timelineEditor->hasSelectionRegion() &&
        timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs() &&
        playbackMs >= timelineEditor->selectionRegionEndMs()) {
        if (timelineEditor->loopEnabled()) {
            playbackMs = timelineEditor->selectionRegionStartMs();
            timelinePlayStartMs = playbackMs;
            timelinePlaybackClock.restart();
            if (playbackPipeline) {
                playbackPipeline->start(playbackMs);
            }
        } else {
            timelinePlaying = false;
            stoppedAtRegionEnd = true;
            timelineTimer.stop();
            stopMediaPlayback();
            timelinePauseButton->setIcon(style()->standardIcon(QStyle::SP_MediaPlay));
            playbackMs = timelineEditor->selectionRegionEndMs();
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
    const auto video = timelineEditor->activeVideoLayersAt(tMs);
    const auto audio = timelineEditor->activeAudioAt(tMs);

    std::vector<PlaybackPipeline::Clip> vclips;
    vclips.reserve(video.size());
    for (const auto &c : video) {
        vclips.push_back({c.sourcePath, c.startMs, c.durationMs, c.sourceInMs});
    }

    std::vector<PlaybackPipeline::Clip> aclips;
    aclips.reserve(audio.size());
    for (const auto &c : audio) {
        aclips.push_back({c.sourcePath, c.startMs, c.durationMs, c.sourceInMs});
    }

    playbackPipeline->update(tMs, vclips, aclips);
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

    const QString outDir =
        QDir::tempPath() + "/kp-draft-" + QDateTime::currentDateTime().toString("yyyyMMdd-hhmmss");
    QDir().mkpath(outDir);

    int frameIndex = 0;
    for (qint64 tMs = startMs; tMs < endMs; tMs += 33) {
        const auto video = timelineEditor->activeVideoLayersAt(tMs);
        std::vector<PlaybackPipeline::Clip> vclips;
        vclips.reserve(video.size());
        for (const auto &c : video) {
            vclips.push_back({c.sourcePath, c.startMs, c.durationMs, c.sourceInMs});
        }
        QImage frame = playbackPipeline->renderDraftFrame(tMs, vclips);
        if (frame.isNull()) {
            frame = QImage(640, 360, QImage::Format_RGB888);
            frame.fill(Qt::black);
        }
        const QString outPath = QString("%1/frame-%2.jpg").arg(outDir, QString::number(frameIndex).rightJustified(6, '0'));
        frame.save(outPath, "JPG", 82);
        ++frameIndex;
    }

    statusBar()->showMessage(
        QString("Draft render complete: %1 frames -> %2").arg(frameIndex).arg(outDir),
        9000);
}

