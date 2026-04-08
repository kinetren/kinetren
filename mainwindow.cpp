#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "playbackpipeline.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"

#include <QCheckBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QMetaObject>
#include <QPushButton>
#include <QScrollBar>
#include <QVBoxLayout>

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

    auto *timelineLayout = new QVBoxLayout(ui->widget_2);
    timelineLayout->setContentsMargins(6, 6, 6, 6);
    timelineLayout->setSpacing(6);

    auto *controlsLayout = new QHBoxLayout();
    timelinePlayPauseButton = new QPushButton("Play/Pause", ui->widget_2);
    auto *markerButton = new QPushButton("Add marker", ui->widget_2);
    auto *addVideoTrackButton = new QPushButton("+ Video Track", ui->widget_2);
    auto *addAudioTrackButton = new QPushButton("+ Audio Track", ui->widget_2);
    auto *splitButton = new QPushButton("Split", ui->widget_2);
    auto *trimStartButton = new QPushButton("Trim Start", ui->widget_2);
    auto *trimEndButton = new QPushButton("Trim End", ui->widget_2);
    auto *moveTrackUpButton = new QPushButton("Track Up", ui->widget_2);
    auto *moveTrackDownButton = new QPushButton("Track Down", ui->widget_2);
    auto *setRegionStartButton = new QPushButton("Set Region In", ui->widget_2);
    auto *setRegionEndButton = new QPushButton("Set Region Out", ui->widget_2);
    auto *clearRegionButton = new QPushButton("Clear Region", ui->widget_2);
    timelineLoopCheckBox = new QCheckBox("Loop Region", ui->widget_2);
    timelineOverlayLabel = new QLabel("Overlay @ 0.00s | Video: - | Audio layers: 0", ui->widget_2);
    controlsLayout->addWidget(timelinePlayPauseButton);
    controlsLayout->addWidget(markerButton);
    controlsLayout->addWidget(addVideoTrackButton);
    controlsLayout->addWidget(addAudioTrackButton);
    controlsLayout->addWidget(splitButton);
    controlsLayout->addWidget(trimStartButton);
    controlsLayout->addWidget(trimEndButton);
    controlsLayout->addWidget(moveTrackUpButton);
    controlsLayout->addWidget(moveTrackDownButton);
    controlsLayout->addWidget(setRegionStartButton);
    controlsLayout->addWidget(setRegionEndButton);
    controlsLayout->addWidget(clearRegionButton);
    controlsLayout->addWidget(timelineLoopCheckBox);
    controlsLayout->addWidget(timelineOverlayLabel, 1);
    timelineLayout->addLayout(controlsLayout);

    timelineEditor = new TimelineEditor(ui->widget_2);
    timelineLayout->addWidget(timelineEditor, 1);
    timelineScrollBar = new QScrollBar(Qt::Horizontal, ui->widget_2);
    timelineLayout->addWidget(timelineScrollBar);

    timelineTimer.setInterval(40);
    connect(&timelineTimer, &QTimer::timeout, this, &MainWindow::onTimelineTick);
    connect(timelinePlayPauseButton, &QPushButton::clicked, this, &MainWindow::toggleTimelinePlayback);
    connect(markerButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::addMarkerAtCursor);
    connect(addVideoTrackButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::addVideoTrack);
    connect(addAudioTrackButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::addAudioTrack);
    connect(splitButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::splitSelectedClipAtCursor);
    connect(trimStartButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::trimSelectedClipStartToCursor);
    connect(trimEndButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::trimSelectedClipEndToCursor);
    connect(moveTrackUpButton, &QPushButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->moveSelectedClipToAdjacentTrack(-1);
        }
    });
    connect(moveTrackDownButton, &QPushButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->moveSelectedClipToAdjacentTrack(1);
        }
    });
    connect(setRegionStartButton, &QPushButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->setSelectionRegionFromCursor(true);
        }
    });
    connect(setRegionEndButton, &QPushButton::clicked, [this]() {
        if (timelineEditor) {
            timelineEditor->setSelectionRegionFromCursor(false);
        }
    });
    connect(clearRegionButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::clearSelectionRegion);
    connect(timelineLoopCheckBox, &QCheckBox::toggled, timelineEditor, &TimelineEditor::setLoopEnabled);
    connect(timelineEditor, &TimelineEditor::loopEnabledChanged, timelineLoopCheckBox, &QCheckBox::setChecked);
    connect(timelineEditor, &TimelineEditor::overlayInfoChanged, timelineOverlayLabel, &QLabel::setText);
    connect(timelineEditor, &TimelineEditor::timelineRangeChanged, this, [this](qint64, qint64) {
        refreshTimelineScroll();
    });
    connect(timelineEditor, &TimelineEditor::viewStartChanged, this, [this](qint64, qint64) {
        refreshTimelineScroll();
    });
    connect(timelineEditor, &TimelineEditor::cursorChanged, this, [this](qint64) {
        refreshTimelineScroll();
    });
    connect(timelineScrollBar, &QScrollBar::valueChanged, [this](int value) {
        if (timelineEditor) {
            timelineEditor->setViewStartMs(static_cast<qint64>(value));
        }
    });
    refreshTimelineScroll();
}

void MainWindow::toggleTimelinePlayback()
{
    if (!timelineEditor) {
        return;
    }

    if (!timelinePlaying) {
        timelinePlaying = true;
        timelineReturnCursorMs = timelineEditor->cursorMs();
        timelinePlayPauseButton->setText("Stop (return)");
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
    timelineTimer.stop();
    stopMediaPlayback();
    timelineEditor->setCursorMs(timelineReturnCursorMs);
    timelinePlayPauseButton->setText("Play/Pause");
}

void MainWindow::onTimelineTick()
{
    if (!timelineEditor || !timelinePlaying) {
        return;
    }
    qint64 playbackMs = timelinePlayStartMs + timelinePlaybackClock.elapsed();
    if (playbackPipeline) {
        playbackMs = std::max(playbackMs, playbackPipeline->playbackPositionMs());
    }
    if (timelineEditor->loopEnabled() && timelineEditor->hasSelectionRegion() &&
        timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs() &&
        playbackMs >= timelineEditor->selectionRegionEndMs()) {
        playbackMs = timelineEditor->selectionRegionStartMs();
        timelinePlayStartMs = playbackMs;
        timelinePlaybackClock.restart();
        if (playbackPipeline) {
            playbackPipeline->start(playbackMs);
        }
    }
    timelineEditor->setCursorMs(playbackMs);
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

