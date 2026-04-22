#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "performancewidget.h"
#include "playbackpipeline.h"
#include "projectrenderer.h"
#include "renderprogressdialog.h"
#include "previewer.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"
#include "sourceeditwidgets.h"

#include <QDockWidget>
#include <QFrame>

#include <QHBoxLayout>
#include <QCheckBox>
#include <QComboBox>
#include <QLabel>
#include <QMessageBox>
#include <QCursor>
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
#include <QTimer>
#include <QtConcurrent/QtConcurrentRun>
#include <QToolButton>
#include <QVBoxLayout>
#include <QSizePolicy>
#include <QPair>
#include <QPointer>
#include <QVector>
#include <QCloseEvent>
#include <QIcon>
#include <QGraphicsDropShadowEffect>
#include <QResizeEvent>

#include <QtGlobal>

#include <algorithm>
#include <memory>
#include <climits>
#include <cmath>
#include <thread>
#include <vector>

namespace {

constexpr int kTimelineUiTickMs = 16;
constexpr int kTimelineVScrollPx = 14;
/// Empty strip under the tool strip so the timeline shows through below the buttons.
constexpr int kTimelineChromeBottomReveal = 12;

/// Editor + vertical scrollbar + bottom tool strip overlaid on the editor (tracks stay visible underneath).
class TimelineStackHost final : public QWidget
{
public:
    explicit TimelineStackHost(QWidget *parent = nullptr)
        : QWidget(parent)
    {
        setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    }

    void setLayers(QWidget *editor, QScrollBar *vsb, QWidget *chrome)
    {
        ed = editor;
        vs = vsb;
        chromeBar = chrome;
        if (ed) {
            ed->setParent(this);
        }
        if (vs) {
            vs->setParent(this);
            vs->setFixedWidth(kTimelineVScrollPx);
        }
        if (chromeBar) {
            chromeBar->setParent(this);
        }
        relayoutChrome();
    }

protected:
    void resizeEvent(QResizeEvent *event) override
    {
        QWidget::resizeEvent(event);
        relayoutChrome();
    }

private:
    void relayoutChrome()
    {
        if (!ed || !vs || !chromeBar) {
            return;
        }
        const int w = width();
        const int h = height();
        const int vsW = vs->width();
        const int ch = std::max(chromeBar->minimumSizeHint().height(), chromeBar->sizeHint().height());
        vs->setGeometry(w - vsW, 0, vsW, h);
        ed->setGeometry(0, 0, w - vsW, h);
        const int chromeY = std::max(0, h - ch - kTimelineChromeBottomReveal);
        chromeBar->setGeometry(0, chromeY, w - vsW, ch);
        chromeBar->raise();
    }

    QWidget *ed = nullptr;
    QScrollBar *vs = nullptr;
    QWidget *chromeBar = nullptr;
};

} // namespace

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(new Ui::MainWindow)
{
    ui->setupUi(this);
    ui->centralwidget->setStyleSheet(QStringLiteral("background-color:#1b1d20;"));
    ui->gridLayout->setContentsMargins(0, 0, 0, 0);
    ui->gridLayout->setSpacing(0);
    setWindowIcon(QIcon(QStringLiteral(":/kp-test.png")));
    setStyleSheet(
        QStringLiteral("QMainWindow { background-color:#1b1d20; }\n"
                       "QMenuBar { background-color:#1b1d20; color:#cfd6df; }\n"
                       "QMenuBar::item:selected { background:#313740; }\n"
                       "QStatusBar { background-color:#1b1d20; color:#8b949e; }\n"
                       "QSplitter::handle { background:#1b1d20; }"));
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
    if (timelineEditor) {
        timelineEditor->setPlaybackMediaPool(playbackPipeline->sharedMediaPool());
    }
    applyProjectSettings();
    performanceTimer.setInterval(500);
    connect(&performanceTimer, &QTimer::timeout, this, &MainWindow::refreshPerformancePanel);
    performanceTimer.start();
    playbackSyncDebounceTimer.setSingleShot(true);
    playbackSyncDebounceTimer.setInterval(33);
    connect(&playbackSyncDebounceTimer, &QTimer::timeout, this, &MainWindow::flushPlaybackSync);
    performanceRefreshDebounceTimer.setSingleShot(true);
    performanceRefreshDebounceTimer.setInterval(120);
    connect(&performanceRefreshDebounceTimer, &QTimer::timeout, this, &MainWindow::flushPerformancePanelRefresh);
    playbackWarmupOverlayTimer.setSingleShot(true);
    connect(&playbackWarmupOverlayTimer, &QTimer::timeout, this, [this]() {
        if (playbackWarmupPending) {
            ui->frame->setBuffering(true);
        }
    });
    syncTimelinePlayback();
    currentProjectPath = defaultAutosaveProjectPath();
    loadLastProjectOnStartup();
    autosaveTimer.setInterval(4000);
    connect(&autosaveTimer, &QTimer::timeout, this, [this]() {
        saveProjectToPath(currentProjectPath, true);
    });
    autosaveTimer.start();
    refreshWindowTitle();
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

void MainWindow::requestCacheCoverageUiRefresh()
{
    refreshPerformancePanel();
}

void MainWindow::setupTimelineEditorUi()
{
    const QScreen *screenRef = QGuiApplication::primaryScreen();
    const qreal dpiScale = screenRef ? std::clamp(screenRef->logicalDotsPerInch() / 96.0, 1.0, 2.4) : 1.0;
    const int pad = qMax(6, qRound(8.0 * dpiScale));
    const int padL = qMax(8, qRound(11.0 * dpiScale));
    const int gap = qMax(4, qRound(6.0 * dpiScale));
    const int frameInset = qMax(2, qRound(3.0 * dpiScale));

    auto *sourceLayout = new QVBoxLayout(ui->widget_3);
    sourceLayout->setContentsMargins(pad, pad, pad, pad);
    sourceLayout->setSpacing(gap);

    auto *sourceTitle = new QLabel("Source area (drop files here, then drag to timeline)", ui->widget_3);
    sourceLayout->addWidget(sourceTitle);

    auto *sourceListFrame = new QFrame(ui->widget_3);
    sourceListFrame->setObjectName(QStringLiteral("SourceListFrame"));
    sourceListFrame->setFrameShape(QFrame::NoFrame);
    sourceListFrame->setStyleSheet(QStringLiteral(
        "#SourceListFrame { background-color:#17191c; border:1px solid #343a42; border-radius:6px; }\n"
        "#SourceListFrame QListWidget {\n"
        "  background-color:#17191c; color:#cfd6df; border:none; outline:none;\n"
        "}\n"
        "#SourceListFrame QListWidget::item { padding:4px 8px; }\n"
        "#SourceListFrame QListWidget::item:selected { background-color:#27435e; }\n"
        "#SourceListFrame QListWidget::item:hover { background-color:#262a2f; }\n"));
    auto *sourceListInner = new QVBoxLayout(sourceListFrame);
    sourceListInner->setContentsMargins(frameInset, frameInset, frameInset, frameInset);
    sourceListInner->setSpacing(0);
    sourceArea = new SourceAreaWidget(sourceListFrame);
    sourceArea->setMinimumHeight(110);
    sourceArea->setMaximumHeight(220);
    sourceListInner->addWidget(sourceArea, 1);
    sourceLayout->addWidget(sourceListFrame, 1);

    performanceWidget = new PerformanceWidget(ui->widget_3);
    performanceWidget->setMinimumHeight(700);
    sourceLayout->addWidget(performanceWidget, 4);
    sourceLayout->setStretch(0, 0); // title
    sourceLayout->setStretch(1, 1); // source list frame
    sourceLayout->setStretch(2, 5); // performance panel

    ui->widget_3->setStyleSheet(
        QStringLiteral(
            "QWidget { background-color:#1b1d20; color:#cfd6df; }\n"
            "QLabel { color:#cfd6df; background-color:transparent; }\n"));

    auto *previewHost = new QWidget(ui->splitter);
    auto *previewLayout = new QVBoxLayout(previewHost);
    previewLayout->setContentsMargins(0, 0, 0, 0);
    previewLayout->setSpacing(0);
    previewHost->setStyleSheet(
        QStringLiteral(
            "QWidget { background-color:#1b1d20; color:#cfd6df; }\n"
            "QLabel { color:#cfd6df; background-color:transparent; }\n"
            "QComboBox, QSpinBox {\n"
            "  background-color:#262a2f; color:#cfd6df; border:1px solid #343a42; border-radius:4px; padding:2px 6px;\n"
            "}\n"
            "QComboBox:hover, QSpinBox:hover { background-color:#313740; border-color:#4b5563; }\n"
            "QCheckBox { color:#cfd6df; spacing:8px; }\n"
            "QCheckBox::indicator { width:14px; height:14px; border:1px solid #343a42; border-radius:3px; "
            "background:#262a2f; }\n"
            "QCheckBox::indicator:checked { background:#27435e; border-color:#5f89ad; }"));
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
    timelineLayout->setContentsMargins(0, 0, pad, qMax(2, qRound(4.0 * dpiScale)));
    timelineLayout->setSpacing(qMax(2, qRound(3.0 * dpiScale)));

    auto *globalCounterWidget = new QWidget(ui->splitter_2);
    globalCounterWidget->setObjectName(QStringLiteral("TimelineTimeStrip"));
    globalCounterWidget->setAttribute(Qt::WA_StyledBackground, true);
    globalCounterWidget->setMinimumHeight(qMax(28, qRound(26.0 * dpiScale)));
    globalCounterWidget->setMaximumHeight(qMax(40, qRound(52.0 * dpiScale)));
    auto *globalCounterLayout = new QHBoxLayout(globalCounterWidget);
    {
        const int pv = qMax(4, qRound(5.0 * dpiScale));
        const int stripLeft = qMax(10, qRound(14.0 * dpiScale));
        globalCounterLayout->setContentsMargins(stripLeft, pv, pad, pv);
    }
    globalCounterLayout->setSpacing(qMax(6, qRound(8.0 * dpiScale)));
    globalCounterWidget->setStyleSheet(QStringLiteral(
        "#TimelineTimeStrip {\n"
        "  background-color:#1b1d20;\n"
        "  color:#cfd6df;\n"
        "  border-bottom:1px solid #343a42;\n"
        "}\n"
        "#TimelineTimeStrip QLabel { color:#cfd6df; background:transparent; }\n"
        "#TimelineTimeStrip QLabel#TimelineGlobalTimeValue {\n"
        "  font-weight:600; color:#d9e3ef;\n"
        "}"));
    timelineGlobalTimeLabel = new QLabel("00:00:00.000 F00", globalCounterWidget);
    timelineGlobalTimeLabel->setObjectName(QStringLiteral("TimelineGlobalTimeValue"));
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

    auto *timelineStackHost = new TimelineStackHost(ui->widget_2);
    timelineLayout->addWidget(timelineStackHost, 1);

    timelineEditor = new TimelineEditor(timelineStackHost);
    timelineScrollBarV = new QScrollBar(Qt::Vertical, timelineStackHost);
    timelineScrollBarV->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    timelineScrollBarV->setFixedWidth(kTimelineVScrollPx);

    auto *timelineChrome = new QWidget(timelineStackHost);
    timelineChrome->setObjectName(QStringLiteral("TimelineToolChrome"));
    timelineChrome->setAttribute(Qt::WA_TranslucentBackground, true);
    auto *controlsLayout = new QHBoxLayout(timelineChrome);
    const int toolChromeLeft = timelineEditor->timelineTrackHeaderWidth() + qMax(6, qRound(8.0 * dpiScale));
    controlsLayout->setContentsMargins(toolChromeLeft, 4, pad, 8);
    controlsLayout->setSpacing(qMax(3, qRound(4.0 * dpiScale)));

    timelineScrollBar = new QScrollBar(Qt::Horizontal, ui->widget_2);
    timelineLayout->addWidget(timelineScrollBar);
    ui->widget_2->setStyleSheet(
        "QWidget { background-color:#1b1d20; color:#cfd6df; }"
        "QLabel { color:#cfd6df; background-color:transparent; }"
        "QToolButton, QPushButton { background-color:#262a2f; color:#cfd6df; border:1px solid #343a42; border-radius:4px; padding:3px 4px; }"
        "QToolButton:hover, QPushButton:hover { background-color:#313740; border-color:#4b5563; }"
        "QToolButton:pressed, QPushButton:pressed { background-color:#3a4350; }"
        "QToolButton:checked, QPushButton:checked { background-color:#27435e; border-color:#5f89ad; }"
        "QScrollBar:horizontal { background-color:#17191c; height:14px; margin:0; border:1px solid #2a2f35; }"
        "QScrollBar::handle:horizontal { background-color:#3a4350; min-width:28px; border-radius:4px; }"
        "QScrollBar::handle:horizontal:hover { background-color:#4a5666; }"
        "QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal { width:0; background:transparent; }"
        "QScrollBar::add-page:horizontal, QScrollBar::sub-page:horizontal { background-color:#1f2328; }"
        "QScrollBar:vertical { background-color:#17191c; width:14px; max-width:14px; min-width:14px; margin:0; border:1px solid #2a2f35; }"
        "QScrollBar::handle:vertical { background-color:#3a4350; min-height:28px; border-radius:4px; }"
        "QScrollBar::handle:vertical:hover { background-color:#4a5666; }"
        "QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height:0; background:transparent; }"
        "QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical { background-color:#1f2328; }"
        "QToolTip { background-color:#22262b; color:#d7dee8; border:1px solid #3b4350; }");
    timelinePauseButton = new QToolButton(timelineChrome);
    auto *markerButton = new QToolButton(timelineChrome);
    auto *addVideoTrackButton = new QToolButton(timelineChrome);
    auto *addAudioTrackButton = new QToolButton(timelineChrome);
    auto *addMorpherTrackButton = new QToolButton(timelineChrome);
    auto *placeMorpherButton = new QToolButton(timelineChrome);
    auto *splitButton = new QToolButton(timelineChrome);
    auto *copyButton = new QToolButton(timelineChrome);
    auto *pasteButton = new QToolButton(timelineChrome);
    auto *trimStartButton = new QToolButton(timelineChrome);
    auto *trimEndButton = new QToolButton(timelineChrome);
    auto *moveTrackUpButton = new QToolButton(timelineChrome);
    auto *moveTrackDownButton = new QToolButton(timelineChrome);
    auto *zoomOutButton = new QToolButton(timelineChrome);
    auto *zoomInButton = new QToolButton(timelineChrome);
    auto *setRegionStartButton = new QToolButton(timelineChrome);
    auto *setRegionEndButton = new QToolButton(timelineChrome);
    auto *clearRegionButton = new QToolButton(timelineChrome);
    auto *renderDraftButton = new QToolButton(timelineChrome);
    timelineLoopButton = new QToolButton(timelineChrome);

    timelinePauseButton->setIcon(style()->standardIcon(QStyle::SP_MediaPlay));
    timelinePauseButton->setToolTip("Play/Pause (Space)");
    markerButton->setIcon(style()->standardIcon(QStyle::SP_DialogYesButton));
    markerButton->setToolTip("Add or remove marker at cursor (M)");
    addVideoTrackButton->setIcon(style()->standardIcon(QStyle::SP_FileDialogNewFolder));
    addVideoTrackButton->setToolTip("Add video track");
    addAudioTrackButton->setIcon(style()->standardIcon(QStyle::SP_DirIcon));
    addAudioTrackButton->setToolTip("Add audio track");
    addMorpherTrackButton->setIcon(style()->standardIcon(QStyle::SP_FileDialogDetailedView));
    addMorpherTrackButton->setToolTip("Add morpher track");
    placeMorpherButton->setIcon(style()->standardIcon(QStyle::SP_ArrowDown));
    placeMorpherButton->setToolTip(
        tr("Add morpher: uses clips in the time region (if drawn) or the selected clip + linked audio/video. "
           "Then click the purple Morph track at the desired time (or drop a file there). Esc cancels."));
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
    configureToolButton(addMorpherTrackButton);
    configureToolButton(placeMorpherButton);
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

    timelineOverlayLabel = new QLabel("Overlay @ 0.00s | Video layers: 0 | Audio layers: 0", timelineChrome);
    controlsLayout->addWidget(timelinePauseButton);
    controlsLayout->addWidget(markerButton);
    controlsLayout->addWidget(addVideoTrackButton);
    controlsLayout->addWidget(addAudioTrackButton);
    controlsLayout->addWidget(addMorpherTrackButton);
    controlsLayout->addWidget(placeMorpherButton);
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

    timelineChrome->setMinimumHeight(buttonH + qMax(12, qRound(14.0 * dpiScale)));
    timelineChrome->setStyleSheet(QStringLiteral(
        "QWidget#TimelineToolChrome {\n"
        "  background: qlineargradient(x1:0,y1:0,x2:0,y2:1,"
        "    stop:0 rgba(27,29,32,238), stop:0.38 rgba(27,29,32,160), stop:0.72 rgba(27,29,32,75), stop:1 rgba(27,29,32,0));\n"
        "  border-top: 1px solid #4a5563;\n"
        "}\n"
        "QWidget#TimelineToolChrome QLabel { color:#cfd6df; background:transparent; }"));
    auto *chromeShadow = new QGraphicsDropShadowEffect(timelineChrome);
    chromeShadow->setBlurRadius(28);
    chromeShadow->setColor(QColor(0, 0, 0, 150));
    chromeShadow->setOffset(0, 7);
    timelineChrome->setGraphicsEffect(chromeShadow);

    timelineStackHost->setLayers(timelineEditor, timelineScrollBarV, timelineChrome);

    timelineTimer.setInterval(kTimelineUiTickMs);
    timelineTimer.setTimerType(Qt::PreciseTimer);
    connect(&timelineTimer, &QTimer::timeout, this, &MainWindow::onTimelineTick);
    connect(timelinePauseButton, &QToolButton::clicked, this, &MainWindow::toggleTimelinePause);
    connect(markerButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addMarkerAtCursor);
    connect(addVideoTrackButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addVideoTrack);
    connect(addAudioTrackButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addAudioTrack);
    connect(addMorpherTrackButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::addMorpherTrack);
    connect(placeMorpherButton, &QToolButton::clicked, timelineEditor, &TimelineEditor::beginMorpherPlacementFromSelection);
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
            if (!timelineEditor->isEditingTimelineContent()) {
                schedulePlaybackSync();
            }
        }
    });
    connect(timelineEditor, &TimelineEditor::timelineRangeChanged, this, [this](qint64, qint64) {
        refreshTimelineScroll();
        if (!timelinePlaying && !timelineEditor->isEditingTimelineContent()) {
            schedulePlaybackSync();
        }
    });
    connect(timelineEditor, &TimelineEditor::viewStartChanged, this, [this](qint64, qint64) {
        refreshTimelineScroll();
    });
    connect(timelineEditor, &TimelineEditor::timelineContentChanged, this, [this]() {
        pruneMorpherWindows();
        refreshTimelineVerticalScroll();
        if (!playbackPipeline) {
            return;
        }
        schedulePlaybackSync();
        schedulePerformancePanelRefresh();
    });

    sourceReplacePanel = new SourceReplacePanel(timelineEditor, this);
    replaceSourceDock = new QDockWidget(tr("Replace source"), this);
    replaceSourceDock->setObjectName(QStringLiteral("ReplaceSourceDock"));
    replaceSourceDock->setWidget(sourceReplacePanel);
    replaceSourceDock->setAllowedAreas(Qt::LeftDockWidgetArea | Qt::RightDockWidgetArea);
    addDockWidget(Qt::RightDockWidgetArea, replaceSourceDock);
    replaceSourceDock->hide();

    sourceOffsetPanel = new SourceOffsetPanel(timelineEditor, this);
    sourceOffsetDock = new QDockWidget(tr("Source offset"), this);
    sourceOffsetDock->setObjectName(QStringLiteral("SourceOffsetDock"));
    sourceOffsetDock->setWidget(sourceOffsetPanel);
    sourceOffsetDock->setAllowedAreas(Qt::LeftDockWidgetArea | Qt::RightDockWidgetArea);
    addDockWidget(Qt::RightDockWidgetArea, sourceOffsetDock);
    sourceOffsetDock->hide();

    const auto styleDockChrome = [](QDockWidget *dock) {
        if (!dock) {
            return;
        }
        dock->setFeatures(QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetClosable |
                          QDockWidget::DockWidgetFloatable);
        dock->setMinimumWidth(320);
        dock->setStyleSheet(
            QStringLiteral(
                "QDockWidget {\n"
                "  font-size: 13px;\n"
                "  color: #e6edf3;\n"
                "}\n"
                "QDockWidget > QWidget {\n"
                "  background: #161b22;\n"
                "}\n"
                "QDockWidget::title {\n"
                "  background: qlineargradient(x1:0, y1:0, x2:0, y2:1,\n"
                "    stop:0 #21262d, stop:1 #161b22);\n"
                "  padding: 11px 14px;\n"
                "  border-bottom: 1px solid #30363d;\n"
                "  border-top-left-radius: 6px;\n"
                "  border-top-right-radius: 6px;\n"
                "  font-weight: 600;\n"
                "}\n"
                "QDockWidget::close-button, QDockWidget::float-button {\n"
                "  width: 24px;\n"
                "  height: 24px;\n"
                "  margin: 4px;\n"
                "  border-radius: 5px;\n"
                "  background: #30363d;\n"
                "}\n"
                "QDockWidget::close-button:hover, QDockWidget::float-button:hover {\n"
                "  background: #484f58;\n"
                "}\n"));
    };
    styleDockChrome(replaceSourceDock);
    styleDockChrome(sourceOffsetDock);

    const auto showReplaceDock = [this]() {
        if (replaceSourceDock) {
            replaceSourceDock->show();
            replaceSourceDock->raise();
        }
    };
    const auto showOffsetDock = [this]() {
        if (sourceOffsetDock) {
            sourceOffsetDock->show();
            sourceOffsetDock->raise();
        }
    };
    connect(timelineEditor, &TimelineEditor::replaceSourcePanelRequestedForClip, this, [showReplaceDock, this](int id) {
        if (sourceReplacePanel) {
            sourceReplacePanel->setClipTarget(id);
        }
        showReplaceDock();
    });
    connect(timelineEditor, &TimelineEditor::replaceSourcePanelRequestedForRegion, this,
             [showReplaceDock, this](bool allTracks, int trackIndex) {
                 if (sourceReplacePanel) {
                     sourceReplacePanel->setRegionTarget(allTracks, trackIndex);
                 }
                 showReplaceDock();
             });
    connect(timelineEditor, &TimelineEditor::morpherEditWindowRequested, this, &MainWindow::openMorpherEditWindow);
    connect(timelineEditor, &TimelineEditor::sourceOffsetPanelRequestedForClip, this, [showOffsetDock, this](int id) {
        if (sourceOffsetPanel) {
            sourceOffsetPanel->setClipTarget(id);
        }
        showOffsetDock();
    });
    connect(timelineEditor, &TimelineEditor::sourceOffsetPanelRequestedForRegion, this,
             [showOffsetDock, this](bool allTracks, int trackIndex) {
                 if (sourceOffsetPanel) {
                     sourceOffsetPanel->setRegionShiftTarget(allTracks, trackIndex);
                 }
                 showOffsetDock();
             });

    connect(timelineScrollBar, &QScrollBar::valueChanged, [this](int value) {
        if (timelineEditor) {
            timelineEditor->setViewStartMs(static_cast<qint64>(value));
        }
    });
    connect(timelineScrollBarV, &QScrollBar::valueChanged, this, [this](int value) {
        if (timelineEditor) {
            timelineEditor->setTimelineVerticalScroll(value);
        }
    });
    connect(timelineEditor, &TimelineEditor::timelineViewLayoutChanged, this, &MainWindow::refreshTimelineVerticalScroll);
    connect(timelineEditor, &TimelineEditor::timelineVerticalScrollChanged, this, [this](int v) {
        if (!timelineScrollBarV) {
            return;
        }
        timelineScrollBarV->blockSignals(true);
        timelineScrollBarV->setValue(v);
        timelineScrollBarV->blockSignals(false);
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
    auto *markerShortcut = new QShortcut(QKeySequence(Qt::Key_M), this);
    connect(markerShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->addMarkerAtCursor();
        }
    });
    auto *splitShortcut = new QShortcut(QKeySequence(Qt::Key_S), this);
    connect(splitShortcut, &QShortcut::activated, [this]() {
        if (!timelineEditor) {
            return;
        }
        if (timelineEditor->hasSelectionRegion() &&
            timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
            timelineEditor->splitDeleteRegionInterior();
        } else {
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
        if (!timelineEditor) {
            return;
        }
        if (timelineEditor->hasSelectionRegion() &&
            timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
            timelineEditor->deleteClipsIntersectingRegion();
        } else {
            timelineEditor->deleteCurrentSelection();
        }
    });
    auto *backspaceShortcut = new QShortcut(QKeySequence(Qt::Key_Backspace), this);
    connect(backspaceShortcut, &QShortcut::activated, [this]() {
        if (!timelineEditor) {
            return;
        }
        if (timelineEditor->hasSelectionRegion() &&
            timelineEditor->selectionRegionEndMs() > timelineEditor->selectionRegionStartMs()) {
            timelineEditor->deleteClipsIntersectingRegion();
        } else {
            timelineEditor->deleteCurrentSelection();
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
    refreshTimelineVerticalScroll();
    refreshPerformancePanel();
    restoreUiState();
}

void MainWindow::saveUiState()
{
    QSettings settings;
    settings.setValue("ui/geometry", saveGeometry());
    if (ui->splitter) {
        settings.setValue("ui/splitterHorizontal", ui->splitter->saveState());
    }
    if (ui->splitter_2) {
        settings.setValue("ui/splitterVertical", ui->splitter_2->saveState());
    }
}

void MainWindow::restoreUiState()
{
    QSettings settings;
    const QByteArray geo = settings.value("ui/geometry").toByteArray();
    if (!geo.isEmpty()) {
        restoreGeometry(geo);
    }
    if (ui->splitter) {
        const QByteArray st = settings.value("ui/splitterHorizontal").toByteArray();
        if (!st.isEmpty()) {
            ui->splitter->restoreState(st);
        }
    }
    if (ui->splitter_2) {
        const QByteArray st = settings.value("ui/splitterVertical").toByteArray();
        if (!st.isEmpty()) {
            ui->splitter_2->restoreState(st);
        }
    }
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
        playbackSyncDebounceTimer.stop();
        performanceRefreshDebounceTimer.stop();
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
            const qint64 cursorMs = timelineEditor->cursorMs();
            playbackPipeline->preparePlaybackAt(cursorMs);
            playbackWarmupSerial += 1;
            const quint64 warmSerial = playbackWarmupSerial;
            if (playbackPipeline->isPlaybackHeadWarm(cursorMs, 5000)) {
                playbackWarmupOverlayTimer.stop();
                playbackWarmupPending = false;
                ui->frame->setBuffering(false);
                playbackPipeline->beginClockAndPrefetch(cursorMs);
                syncTimelinePlayback();
                timelineTimer.start();
                requestCacheCoverageUiRefresh();
            } else {
                playbackWarmupPending = true;
                playbackWarmupOverlayTimer.start(180);
                QPointer<MainWindow> warmGuard(this);
                (void)QtConcurrent::run([warmGuard, warmSerial, cursorMs]() {
                    if (!warmGuard) {
                        return;
                    }
                    PlaybackPipeline *pl = warmGuard->playbackPipeline.get();
                    if (!pl) {
                        return;
                    }
                    pl->warmPlaybackHeadBlocking(cursorMs, 5000);
                    QMetaObject::invokeMethod(
                        warmGuard.data(),
                        [warmGuard, warmSerial, cursorMs]() {
                            if (!warmGuard) {
                                return;
                            }
                            warmGuard->playbackWarmupOverlayTimer.stop();
                            warmGuard->playbackWarmupPending = false;
                            warmGuard->ui->frame->setBuffering(false);
                            if (warmSerial != warmGuard->playbackWarmupSerial) {
                                return;
                            }
                            if (!warmGuard->timelinePlaying) {
                                warmGuard->stopMediaPlayback();
                                return;
                            }
                            if (!warmGuard->playbackPipeline) {
                                return;
                            }
                            warmGuard->playbackPipeline->beginClockAndPrefetch(cursorMs);
                            warmGuard->syncTimelinePlayback();
                            warmGuard->timelineTimer.start();
                            warmGuard->requestCacheCoverageUiRefresh();
                        },
                        Qt::QueuedConnection);
                });
            }
        }
        syncTimelinePlayback();
        if (!playbackPipeline) {
            timelineTimer.start();
        }
        return;
    }

    timelinePlaying = false;
    playbackWarmupSerial += 1;
    playbackWarmupOverlayTimer.stop();
    playbackWarmupPending = false;
    if (ui->frame) {
        ui->frame->setBuffering(false);
    }
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

void MainWindow::schedulePlaybackSync()
{
    if (!playbackPipeline || !timelineEditor) {
        return;
    }
    playbackSyncDebounceTimer.start();
}

void MainWindow::flushPlaybackSync()
{
    if (!playbackPipeline || !timelineEditor) {
        return;
    }
    if (timelinePlaying) {
        return;
    }
    syncTimelinePlayback();
}

void MainWindow::schedulePerformancePanelRefresh()
{
    if (!playbackPipeline || !performanceWidget || !timelineEditor) {
        return;
    }
    performanceRefreshDebounceTimer.start();
}

void MainWindow::flushPerformancePanelRefresh()
{
    refreshPerformancePanel();
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

void MainWindow::refreshTimelineVerticalScroll()
{
    if (!timelineEditor || !timelineScrollBarV) {
        return;
    }
    const int contentH = timelineEditor->timelineContentHeightPx();
    const int viewH = timelineEditor->height();
    const int maxScroll = std::max(0, contentH - viewH);
    timelineScrollBarV->blockSignals(true);
    timelineScrollBarV->setVisible(maxScroll > 0);
    timelineScrollBarV->setRange(0, maxScroll);
    timelineScrollBarV->setPageStep(std::max(1, viewH - 24));
    timelineScrollBarV->setSingleStep(20);
    const int clamped = std::clamp(timelineEditor->timelineVerticalScroll(), 0, maxScroll);
    timelineScrollBarV->setValue(clamped);
    timelineScrollBarV->blockSignals(false);
    if (clamped != timelineEditor->timelineVerticalScroll()) {
        timelineEditor->setTimelineVerticalScroll(clamped);
    }
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

    QPointer<RenderProgressDialog> dlg(new RenderProgressDialog(this));
    dlg->setExportInfo(projectSettings);
    dlg->appendLog(tr("Output file: %1").arg(outPath));
    dlg->setRunning(true);
    dlg->show();

    renderInProgress = true;
    const auto clips = timelineEditor->renderClips();
    const ProjectSettings settings = projectSettings;
    QPointer<MainWindow> guard(this);
    QPointer<TimelineEditor> timelinePtr(timelineEditor);
    const std::shared_ptr<std::atomic<bool>> cancelPtr = dlg->cancelFlag();
    const std::shared_ptr<std::atomic<bool>> pausePtr = dlg->pauseFlag();

    std::thread([guard, dlg, timelinePtr, clips, settings, startMs, endMs, outPath, cancelPtr, pausePtr]() {
        const auto result = ProjectRenderer::renderMp4(
            clips,
            settings,
            startMs,
            endMs,
            outPath,
            [guard, timelinePtr, dlg](const ProjectRenderer::RenderProgress &p) {
                if (!guard || !dlg || !timelinePtr) {
                    return;
                }
                const ProjectRenderer::RenderProgress snapshot = p;
                QMetaObject::invokeMethod(
                    guard.data(),
                    [guard, timelinePtr, dlg, snapshot]() {
                        if (!guard || !dlg || !timelinePtr) {
                            return;
                        }
                        int bar = 0;
                        if (snapshot.phase == ProjectRenderer::RenderProgress::Phase::VideoFrames) {
                            bar = (snapshot.done * 900) / std::max(1, snapshot.total);
                        } else if (snapshot.phase == ProjectRenderer::RenderProgress::Phase::AudioPcm) {
                            bar = 900 + (snapshot.done * 90) / std::max(1, snapshot.total);
                        } else {
                            bar = snapshot.done >= snapshot.total ? 1000 : 995;
                        }
                        dlg->setProgress(std::min(1000, bar), 1000);
                        QString stage;
                        switch (snapshot.phase) {
                        case ProjectRenderer::RenderProgress::Phase::VideoFrames:
                            stage = QStringLiteral("Video");
                            break;
                        case ProjectRenderer::RenderProgress::Phase::AudioPcm:
                            stage = QStringLiteral("Audio (PCM)");
                            break;
                        case ProjectRenderer::RenderProgress::Phase::Muxing:
                            stage = QStringLiteral("ffmpeg mux");
                            break;
                        }
                        qint64 remainingMs = 0;
                        if (snapshot.done > 0 && snapshot.done < snapshot.total) {
                            remainingMs = (snapshot.elapsedMs * (snapshot.total - snapshot.done)) / snapshot.done;
                        }
                        const QString remStr =
                            remainingMs > 0 ? timelinePtr->formatTimelineTime(remainingMs) : QStringLiteral("—");
                        dlg->setProgressView(stage,
                                             snapshot.done,
                                             snapshot.total,
                                             timelinePtr->formatTimelineTime(snapshot.elapsedMs),
                                             remStr,
                                             timelinePtr->formatTimelineTime(snapshot.outputDurationMs),
                                             snapshot.speedRealtimeFactor);
                    },
                    Qt::QueuedConnection);
            },
            cancelPtr.get(),
            pausePtr.get(),
            [guard, dlg](const QString &line) {
                if (!guard || !dlg) {
                    return;
                }
                const QString copy = line;
                QMetaObject::invokeMethod(
                    guard.data(),
                    [dlg, copy]() {
                        if (dlg) {
                            dlg->appendLog(copy);
                        }
                    },
                    Qt::QueuedConnection);
            });
        if (!guard) {
            return;
        }
        QMetaObject::invokeMethod(
            guard.data(),
            [guard, dlg, result]() {
                if (!guard) {
                    return;
                }
                guard->renderInProgress = false;
                if (result.cancelled) {
                    dlg->appendLog(result.message);
                    dlg->setFinished(false, QString());
                    guard->statusBar()->showMessage(result.message, 6000);
                    return;
                }
                if (!result.ok) {
                    QString brief = QStringLiteral("Render failed: ") + result.message;
                    if (!result.logPath.isEmpty()) {
                        brief += QStringLiteral(" (log: ") + result.logPath + QStringLiteral(")");
                    }
                    guard->statusBar()->showMessage(brief, 20000);
                    dlg->appendLog(QStringLiteral("---"));
                    dlg->appendLog(result.message);
                    if (!result.details.isEmpty()) {
                        dlg->appendLog(result.details);
                    }
                    if (!result.logPath.isEmpty()) {
                        dlg->appendLog(tr("Saved log: %1").arg(result.logPath));
                    }
                    dlg->setFinished(false, result.message);
                    return;
                }
                dlg->setFinished(true, result.message);
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
    const auto clipVideoSliding = playbackPipeline->videoFrameSlidingSegments(videoRenderClips);
    const auto clipVideoHead = playbackPipeline->videoFrameHeadSegments(videoRenderClips);
    const auto clipAudio = playbackPipeline->audioCacheSegments(audioRenderClips);
    const qint64 viewStart = timelineEditor->viewStartMs();
    const qint64 viewEnd = viewStart + timelineEditor->visibleDurationMs();
    const auto timelineVideoSliding = playbackPipeline->timelineVideoSlidingSegments(viewStart, viewEnd);
    const auto timelineVideoHead = playbackPipeline->timelineVideoHeadSegments(viewStart, viewEnd);
    if (!timelineEditor->isEditingTimelineContent()) {
        timelineEditor->setCacheCoverage(clipVideoSliding, clipVideoHead, timelineVideoSliding, timelineVideoHead, clipAudio);
    }

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
    refreshWindowTitle();
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
    refreshWindowTitle();
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

namespace {

void placeMorpherWindowTitlebarUnderCursor(MorpherEditWindow *w)
{
    if (!w || !w->isVisible()) {
        return;
    }
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen) {
        screen = QGuiApplication::primaryScreen();
    }
    const QRect avail = screen->availableGeometry();

    const QRect fr = w->frameGeometry();
    const QRect gr = w->geometry();
    int titleDepth = gr.top() - fr.top();
    if (titleDepth <= 0) {
        titleDepth = w->style()->pixelMetric(QStyle::PM_TitleBarHeight, nullptr, w);
        titleDepth = std::max(titleDepth, 24);
    }
    const QPoint anchor(fr.left() + fr.width() / 2, fr.top() + titleDepth / 2);

    const QPoint d = QCursor::pos() - anchor;
    w->move(w->pos() + d);

    QRect f = w->frameGeometry();
    int dx = 0;
    int dy = 0;
    if (f.left() < avail.left()) {
        dx = avail.left() - f.left();
    } else if (f.right() > avail.right()) {
        dx = avail.right() - f.right();
    }
    if (f.top() < avail.top()) {
        dy = avail.top() - f.top();
    } else if (f.bottom() > avail.bottom()) {
        dy = avail.bottom() - f.bottom();
    }
    if (dx != 0 || dy != 0) {
        w->move(w->pos() + QPoint(dx, dy));
    }
}

} // namespace

void MainWindow::openMorpherEditWindow(int morpherEventId)
{
    if (!timelineEditor || morpherEventId < 0 || !timelineEditor->hasMorpherEvent(morpherEventId)) {
        return;
    }
    if (MorpherEditWindow *existing = morpherEditWindows.value(morpherEventId)) {
        existing->raise();
        existing->activateWindow();
        existing->reloadFromTimeline();
        return;
    }
    auto *win = new MorpherEditWindow(timelineEditor, morpherEventId, this);
    morpherEditWindows.insert(morpherEventId, win);
    connect(win, &QObject::destroyed, this, [this, morpherEventId]() {
        morpherEditWindows.remove(morpherEventId);
    });
    win->show();
    QPointer<MorpherEditWindow> winGuard(win);
    QTimer::singleShot(0, win, [winGuard]() {
        if (winGuard) {
            placeMorpherWindowTitlebarUnderCursor(winGuard.data());
        }
    });
}

void MainWindow::pruneMorpherWindows()
{
    if (!timelineEditor) {
        return;
    }
    const QList<int> ids = morpherEditWindows.keys();
    for (int id : ids) {
        if (!timelineEditor->hasMorpherEvent(id)) {
            if (MorpherEditWindow *w = morpherEditWindows.value(id)) {
                w->close();
            }
        }
    }
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
    refreshWindowTitle();
}

void MainWindow::refreshWindowTitle()
{
    const QString base = QFileInfo(currentProjectPath).fileName();
    setWindowTitle(QStringLiteral("KP: %1").arg(base.isEmpty() ? QStringLiteral("untitled") : base));
}

void MainWindow::changeEvent(QEvent *event)
{
    if (event->type() == QEvent::WindowActivate) {
        const QList<MorpherEditWindow *> morphers = morpherEditWindows.values();
        for (MorpherEditWindow *w : morphers) {
            if (w && w->isVisible() && !w->isMinimized()) {
                w->raise();
            }
        }
    }
    QMainWindow::changeEvent(event);
}

void MainWindow::closeEvent(QCloseEvent *event)
{
    saveUiState();
    saveProjectToPath(currentProjectPath, true);
    QMainWindow::closeEvent(event);
}

