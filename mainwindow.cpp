#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "playbackpipeline.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QMetaObject>
#include <QPushButton>
#include <QVBoxLayout>

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
    timelinePlayPauseButton = new QPushButton("Pause", ui->widget_2);
    auto *markerButton = new QPushButton("Add marker", ui->widget_2);
    timelineOverlayLabel = new QLabel("Overlay @ 0.00s | Video: - | Audio layers: 0", ui->widget_2);
    controlsLayout->addWidget(timelinePlayPauseButton);
    controlsLayout->addWidget(markerButton);
    controlsLayout->addWidget(timelineOverlayLabel, 1);
    timelineLayout->addLayout(controlsLayout);

    timelineEditor = new TimelineEditor(ui->widget_2);
    timelineLayout->addWidget(timelineEditor, 1);

    timelineTimer.setInterval(40);
    connect(&timelineTimer, &QTimer::timeout, this, &MainWindow::onTimelineTick);
    connect(timelinePlayPauseButton, &QPushButton::clicked, this, &MainWindow::toggleTimelinePlayback);
    connect(markerButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::addMarkerAtCursor);
    connect(timelineEditor, &TimelineEditor::overlayInfoChanged, timelineOverlayLabel, &QLabel::setText);
}

void MainWindow::toggleTimelinePlayback()
{
    if (!timelineEditor) {
        return;
    }

    if (!timelinePlaying) {
        timelinePlaying = true;
        timelineReturnCursorMs = timelineEditor->cursorMs();
        timelinePlayPauseButton->setText("Pause (back)");
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
    timelinePlayPauseButton->setText("Pause");
}

void MainWindow::onTimelineTick()
{
    if (!timelineEditor || !timelinePlaying) {
        return;
    }
    timelineEditor->setCursorMs(timelineEditor->cursorMs() + timelineTimer.interval());
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
        vclips.push_back({c.sourcePath, c.startMs, c.durationMs});
    }

    std::vector<PlaybackPipeline::Clip> aclips;
    aclips.reserve(audio.size());
    for (const auto &c : audio) {
        aclips.push_back({c.sourcePath, c.startMs, c.durationMs});
    }

    playbackPipeline->update(tMs, vclips, aclips);
}

void MainWindow::stopMediaPlayback()
{
    if (playbackPipeline) {
        playbackPipeline->stop();
    }
}

