#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "bc.h"
#include "performancewidget.h"
#include "performancediagnosticswindow.h"
#include "playbackpipeline.h"
#include "projectrenderer.h"
#include "renderprogressdialog.h"
#include "audiolevelmeterwidget.h"
#include "previewer.h"
#include "previewaudiopanel.h"
#include "previewvolumefader.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"
#include "clipfxchaindialog.h"
#include "sourceeditwidgets.h"

#include <QAbstractItemModel>
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
#include <QSet>
#include <QVariant>
#include <QPushButton>
#include <QShortcut>
#include <QScreen>
#include <QScrollBar>
#include <QSignalBlocker>
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
#include <cmath>
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

namespace {

int cacheQualityToComboIndex(ProjectSettings::CacheQuality q)
{
    switch (q) {
    case ProjectSettings::CacheQuality::Quarter16:
        return 0;
    case ProjectSettings::CacheQuality::Quarter8:
        return 1;
    case ProjectSettings::CacheQuality::Quarter4:
        return 2;
    case ProjectSettings::CacheQuality::Half:
        return 3;
    case ProjectSettings::CacheQuality::Source:
    default:
        return 4;
    }
}

ProjectSettings::CacheQuality comboIndexToCacheQuality(int index)
{
    switch (index) {
    case 0:
        return ProjectSettings::CacheQuality::Quarter16;
    case 1:
        return ProjectSettings::CacheQuality::Quarter8;
    case 2:
        return ProjectSettings::CacheQuality::Quarter4;
    case 3:
        return ProjectSettings::CacheQuality::Half;
    case 4:
    default:
        return ProjectSettings::CacheQuality::Source;
    }
}

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
            Previewer *frameView = ui->frame;
            if (!frameView) {
                return;
            }
            // Queued slot + Q_ARG is the most reliable cross-thread path to the GUI thread (functor
            // invokeMethod / singleShot coalescing can drop frames under load).
            QMetaObject::invokeMethod(
                frameView,
                "submitFrame",
                Qt::QueuedConnection,
                Q_ARG(QImage, image));
        });
    {
        burstcon::ConMediaPoolConfig stripCfg;
        stripCfg.enableConMediaCache = true;
        stripCfg.cacheConfig.maxEntries = 160;
        stripCfg.cacheConfig.maxAudioEntries = 96;
        stripCfg.cacheConfig.maxVideoEntries = 72;
        stripCfg.cacheConfig.maxVideoBytes = 96ULL * 1024ULL * 1024ULL;
        stripCfg.cacheConfig.preloadEnabled = false;
        stripCfg.forcedPreviewQualityEnabled = true;
        stripCfg.forcedPreviewQuality = burstcon::BurstQuality{320, 180};
        stripCfg.previewParallelShards = 8;
        stripCfg.maxMediaInstances = 64;
        stripMediaPool = std::make_shared<burstcon::ConMediaPool>(stripCfg);
    }
    if (timelineEditor) {
        timelineEditor->setPlaybackMediaPool(playbackPipeline->sharedMediaPool());
        timelineEditor->setStripMediaPool(stripMediaPool);
    }
    playbackPipeline->setAudioLevelsCallback([this](float dbL, float dbR) {
        QMetaObject::invokeMethod(
            this,
            [this, dbL, dbR]() {
                if (mainAudioMeters) {
                    mainAudioMeters->pushLevelsDb(dbL, dbR);
                }
            },
            Qt::QueuedConnection);
    });
    if (previewVolumeFader && playbackPipeline) {
        float lin = 0.f;
        if (!previewVolumeFader->isMuted()) {
            lin = std::pow(10.f, previewVolumeFader->volumeDb() / 20.f);
        }
        playbackPipeline->setPreviewVolume(lin);
    }
    refreshPreviewResolutionMaxEntry();
    if (frontScalerCheck) {
        connect(frontScalerCheck, &QCheckBox::toggled, this, [this](bool on) {
            projectSettings.scaleVideoAtDisplayTime = on;
            if (on) {
                applyRecommendedPreviewCacheQualityForCap();
            }
            applyProjectSettings();
            if (playbackPipeline) {
                playbackPipeline->resetProjectCache();
            }
            if (timelineEditor) {
                timelineEditor->purgePresentationCaches();
            }
            requestCacheCoverageUiRefresh();
            if (!timelinePlaying) {
                syncTimelinePlayback();
            }
        });
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
    mediaSourceProbeTimer.setInterval(900);
    connect(&mediaSourceProbeTimer, &QTimer::timeout, this, &MainWindow::tickMediaSourceAvailability);
    mediaSourceProbeTimer.start();
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

    if (QAbstractItemModel *srcModel = sourceArea->model()) {
        connect(srcModel, &QAbstractItemModel::rowsInserted, this, [this](const QModelIndex &, int, int) {
            if (playbackPipeline) {
                refreshPreviewResolutionMaxEntry();
            }
        });
        connect(srcModel, &QAbstractItemModel::rowsRemoved, this, [this](const QModelIndex &, int, int) {
            if (playbackPipeline) {
                refreshPreviewResolutionMaxEntry();
            }
        });
        connect(srcModel, &QAbstractItemModel::modelReset, this, [this]() {
            if (playbackPipeline) {
                refreshPreviewResolutionMaxEntry();
            }
        });
    }

    performanceWidget = new PerformanceWidget(ui->widget_3);
    performanceWidget->setMinimumHeight(700);
    connect(performanceWidget,
            &PerformanceWidget::diagnosticsWindowRequested,
            this,
            &MainWindow::openPerformanceDiagnosticsWindow);
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
    auto *previewMediaRow = new QWidget(previewHost);
    auto *previewMediaLayout = new QHBoxLayout(previewMediaRow);
    previewMediaLayout->setContentsMargins(0, 0, 0, 0);
    previewMediaLayout->setSpacing(4);
    previewMediaLayout->addWidget(ui->frame, 1);

    previewAudioPanel = new PreviewAudioPanel(previewMediaRow);
    auto *audioMetersHBox = new QHBoxLayout(previewAudioPanel->contentRow());
    audioMetersHBox->setContentsMargins(0, 0, 0, 0);
    audioMetersHBox->setSpacing(4);
    mainAudioMeters = new AudioLevelMeterWidget(previewAudioPanel->contentRow());
    mainAudioMeters->setMinimumWidth(112);
    audioMetersHBox->addWidget(mainAudioMeters, 0);
    previewVolumeFader = new PreviewVolumeFader(previewAudioPanel->contentRow());
    audioMetersHBox->addWidget(previewVolumeFader, 0);
    previewMediaLayout->addWidget(previewAudioPanel, 0);
    previewLayout->addWidget(previewMediaRow, 1);

    auto *previewControls = new QHBoxLayout();
    auto *previewQualityLabel = new QLabel("Cache quality:", previewHost);
    previewCacheQualityCombo = new QComboBox(previewHost);
    previewCacheQualityCombo->addItem("1/16");
    previewCacheQualityCombo->addItem("1/8");
    previewCacheQualityCombo->addItem("1/4");
    previewCacheQualityCombo->addItem("1/2");
    previewCacheQualityCombo->addItem("1/1 (src)");
    previewCacheQualityCombo->setCurrentIndex(cacheQualityToComboIndex(projectSettings.cacheQuality));
    auto *fpsLabel = new QLabel("FPS:", previewHost);
    previewFpsCombo = new QComboBox(previewHost);
    previewFpsCombo->addItem("24");
    previewFpsCombo->addItem("25");
    previewFpsCombo->addItem("30");
    previewFpsCombo->addItem("60");
    previewFpsCombo->setCurrentText(QString::number(projectSettings.fps));
    auto *seekLookbackLabel = new QLabel("Read step:", previewHost);
    previewSeekLookbackSpin = new QSpinBox(previewHost);
    previewSeekLookbackSpin->setRange(0, 2000);
    previewSeekLookbackSpin->setSingleStep(1);
    previewSeekLookbackSpin->setSuffix(" ms");
    previewSeekLookbackSpin->setSpecialValueText("Auto (1 frame)");
    previewSeekLookbackSpin->setToolTip("How far back we seek before decoding. Auto = 1 frame at current FPS.");
    previewSeekLookbackSpin->setValue(std::max(0, projectSettings.decodeSeekLookbackMs));
    auto *resolutionLabel = new QLabel("Resolution:", previewHost);
    previewResolutionCombo = new QComboBox(previewHost);
    previewResolutionCombo->addItem(QStringLiteral("640×360"), QSize(640, 360));
    previewResolutionCombo->addItem(QStringLiteral("1280×720"), QSize(1280, 720));
    previewResolutionCombo->addItem(QStringLiteral("1920×1080"), QSize(1920, 1080));
    previewResolutionCombo->setCurrentIndex(1);
    previewSmoothScalingCheck = new QCheckBox("Smooth scaling", previewHost);
    previewSmoothScalingCheck->setChecked(projectSettings.previewSmoothScaling);
    frontScalerCheck = new QCheckBox(QStringLiteral("FrontScaler"), previewHost);
    frontScalerCheck->setChecked(projectSettings.scaleVideoAtDisplayTime);
    frontScalerCheck->setToolTip(
        QStringLiteral("Decode video near the project cache resolution, then let the preview widget scale "
                       "the RGB image in Qt. libswscale still converts YUV→RGB at that decode size "
                       "(you will still see ff_yuv_420_rgb24_* in profiles, but with fewer pixels than "
                       "full-res decode). When off, decode at full source size then Qt scales (heavier). "
                       "Toggling clears playback and strip caches so sizes stay consistent."));
    previewAntialiasingCheck = new QCheckBox("Antialiasing", previewHost);
    previewAntialiasingCheck->setChecked(projectSettings.previewAntialiasing);
    if (ui->frame) {
        ui->frame->setSmoothScaling(projectSettings.previewSmoothScaling);
        ui->frame->setAntialiasing(projectSettings.previewAntialiasing);
    }
    previewControls->addWidget(previewQualityLabel);
    previewControls->addWidget(previewCacheQualityCombo);
    previewControls->addWidget(fpsLabel);
    previewControls->addWidget(previewFpsCombo);
    previewControls->addWidget(seekLookbackLabel);
    previewControls->addWidget(previewSeekLookbackSpin);
    previewControls->addWidget(resolutionLabel);
    previewControls->addWidget(previewResolutionCombo);
    previewControls->addSpacing(10);
    previewControls->addWidget(frontScalerCheck);
    previewControls->addWidget(previewSmoothScalingCheck);
    previewControls->addWidget(previewAntialiasingCheck);
    previewControls->addStretch(1);
    previewLayout->addLayout(previewControls);

    connect(previewVolumeFader, &PreviewVolumeFader::volumeChanged, this, [this](float linear) {
        if (playbackPipeline) {
            playbackPipeline->setPreviewVolume(linear);
        }
    });
    auto refreshPreviewVolumeOverlay = [this]() {
        if (!previewAudioPanel || !previewVolumeFader) {
            return;
        }
        if (previewVolumeFader->isMuted()) {
            previewAudioPanel->setVolumeOverlayText(tr("MUTE"));
        } else {
            const float db = previewVolumeFader->volumeDb();
            previewAudioPanel->setVolumeOverlayText(
                QStringLiteral("%1 dB").arg(db, 0, 'f', db >= 10.f ? 0 : 1));
        }
    };
    connect(previewVolumeFader, &PreviewVolumeFader::volumeDbChanged, this,
            [refreshPreviewVolumeOverlay](float) { refreshPreviewVolumeOverlay(); });
    connect(previewVolumeFader, &PreviewVolumeFader::activityChanged, this,
            [refreshPreviewVolumeOverlay](bool) { refreshPreviewVolumeOverlay(); });
    refreshPreviewVolumeOverlay();

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
    auto *placeMorpherFxBatchButton = new QToolButton(timelineChrome);
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
    placeMorpherFxBatchButton->setIcon(style()->standardIcon(QStyle::SP_CommandLink));
    placeMorpherFxBatchButton->setToolTip(
        tr("Add morpher FX batch: same target locking as morpher, but the block stores VFX/AFX to append to all "
           "frozen targets when you bake (replace source is not used)."));
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
    configureToolButton(placeMorpherFxBatchButton);
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
    controlsLayout->addWidget(placeMorpherFxBatchButton);
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
    connect(placeMorpherFxBatchButton, &QToolButton::clicked, timelineEditor,
            &TimelineEditor::beginMorpherEffectBatchPlacementFromSelection);
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
    connect(timelineEditor, &TimelineEditor::clipVideoFxEditorRequested, this,
            [this](int clipId, const QString &hint) { openClipFxChainWindow(clipId, ClipFxChainWindow::MediaKind::Video, hint); });
    connect(timelineEditor, &TimelineEditor::clipAudioFxEditorRequested, this, [this](int clipId) {
        openClipFxChainWindow(clipId, ClipFxChainWindow::MediaKind::Audio, QString());
    });
    connect(timelineEditor, &TimelineEditor::playheadScrubStarted, this, [this]() {
        if (!timelinePlaying && playbackPipeline && timelineEditor) {
            flushPlaybackSync();
            playbackPipeline->beginScrubPlayback(timelineEditor->cursorMs());
        }
    });
    connect(timelineEditor, &TimelineEditor::playheadScrubEnded, this, [this]() {
        if (playbackPipeline) {
            playbackPipeline->endScrubPlayback();
            flushPlaybackSync();
        }
    });
    connect(timelineEditor, &TimelineEditor::cursorChanged, this, [this](qint64 t) {
        if (!timelinePlaying) {
            refreshTimelineScroll();
            if (timelineGlobalTimeLabel && timelineEditor) {
                timelineGlobalTimeLabel->setText(timelineEditor->formatTimelineTime(t));
            }
            if (!timelineEditor->isEditingTimelineContent()) {
                pendingAudibleCursorPreview = true;
                if (timelineEditor->isScrubbingPlayhead()) {
                    flushPlaybackSync();
                    if (playbackPipeline) {
                        playbackPipeline->scrubPlaybackSeek(t);
                    }
                } else {
                    schedulePlaybackSync();
                }
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
        pruneClipFxWindows();
        refreshTimelineVerticalScroll();
        if (playbackPipeline) {
            refreshPreviewResolutionMaxEntry();
        }
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
    connect(previewCacheQualityCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int index) {
        projectSettings.cacheQuality = comboIndexToCacheQuality(index);
        applyProjectSettings();
        if (!timelinePlaying) {
            syncTimelinePlayback();
        }
    });
    connect(previewFpsCombo, &QComboBox::currentTextChanged, this, [this](const QString &text) {
        bool ok = false;
        const int fps = text.toInt(&ok);
        if (!ok || fps <= 0) {
            return;
        }
        projectSettings.fps = fps;
        applyProjectSettings();
    });
    connect(previewSeekLookbackSpin, QOverload<int>::of(&QSpinBox::valueChanged), this, [this](int value) {
        projectSettings.decodeSeekLookbackMs = std::max(0, value);
        applyProjectSettings();
    });
    connect(previewResolutionCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int index) {
        if (!previewResolutionCombo || index < 0) {
            return;
        }
        const QVariant v = previewResolutionCombo->itemData(index);
        if (!v.canConvert<QSize>()) {
            return;
        }
        const QSize sz = v.value<QSize>();
        if (!sz.isValid() || sz.width() <= 0 || sz.height() <= 0) {
            return;
        }
        projectSettings.resolution = sz;
        if (projectSettings.scaleVideoAtDisplayTime) {
            applyRecommendedPreviewCacheQualityForCap();
        }
        applyProjectSettings();
        if (!timelinePlaying) {
            syncTimelinePlayback();
        }
    });
    connect(previewSmoothScalingCheck, &QCheckBox::toggled, this, [this](bool on) {
        projectSettings.previewSmoothScaling = on;
        if (ui->frame) {
            ui->frame->setSmoothScaling(on);
        }
    });
    connect(previewAntialiasingCheck, &QCheckBox::toggled, this, [this](bool on) {
        projectSettings.previewAntialiasing = on;
        if (ui->frame) {
            ui->frame->setAntialiasing(on);
        }
    });

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
    auto *undoShortcut = new QShortcut(QKeySequence::Undo, this);
    connect(undoShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->undo();
        }
    });
    auto *redoShortcut = new QShortcut(QKeySequence::Redo, this);
    connect(redoShortcut, &QShortcut::activated, [this]() {
        if (timelineEditor) {
            timelineEditor->redo();
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
    if (previewVolumeFader) {
        settings.setValue(QStringLiteral("ui/previewVolumeDb"), static_cast<double>(previewVolumeFader->volumeDb()));
        settings.setValue(QStringLiteral("ui/previewVolumeMuted"), previewVolumeFader->isMuted());
    }
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
    if (previewVolumeFader) {
        const float db = static_cast<float>(settings.value(QStringLiteral("ui/previewVolumeDb"), 0.0).toDouble());
        previewVolumeFader->setVolumeDb(db);
        if (settings.value(QStringLiteral("ui/previewVolumeMuted"), false).toBool()) {
            previewVolumeFader->setMuted(true);
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
            if (playbackPipeline->isScrubPlaybackActive()) {
                playbackPipeline->endScrubPlayback();
            }
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
            // Rebuild trimmer/video graph from the timeline *before* arming the clock — otherwise
            // MonotonicPlaybackTimer can decode against an empty or stale graph for many frames.
            syncTimelinePlayback();
            const qint64 cursorMs = timelineEditor->cursorMs();
            playbackPipeline->preparePlaybackAt(cursorMs);
            playbackWarmupSerial += 1;
            const quint64 warmSerial = playbackWarmupSerial;
            if (playbackPipeline->isPlaybackHeadWarm(cursorMs, 5000)) {
                playbackWarmupOverlayTimer.stop();
                playbackWarmupPending = false;
                ui->frame->setBuffering(false);
                playbackPipeline->beginClockAndPrefetch(cursorMs);
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
    refreshMainAudioMeters();
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
        PlaybackPipeline::Clip clip;
        clip.sourcePath = c.sourcePath;
        clip.startMs = c.startMs;
        clip.durationMs = c.durationMs;
        clip.sourceInMs = c.sourceInMs;
        clip.trackIndex = c.trackIndex;
        clip.videoFxChain = c.videoFxChain;
        clip.audioFxChain = c.audioFxChain;
        clip.audioLinearGain = c.audioLinearGain;
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
    // Always push the latest clip FX chains into the pipeline. Skipping while `timelinePlaying` was
    // wrong: the UI timer can lag a tick, and debounced updates (timelineContentChanged) must still
    // refresh the graph so ConTrack picks up new `videoFxChain` / `audioFxChain` JSON.
    if (!timelinePlaying && !playbackPipeline->isScrubPlaybackActive()) {
        playbackPipeline->ensureIdlePreviewAudio();
    }
    syncTimelinePlayback();
    refreshMainAudioMeters();
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

void MainWindow::refreshMainAudioMeters()
{
    if (!timelineEditor || !playbackPipeline || timelinePlaying) {
        return;
    }
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    const qint64 sampleIntervalMs =
        timelineEditor->isScrubbingPlayhead() ? 35 : 120;
    if (now - lastAudioMeterSampleMs < sampleIntervalMs) {
        return;
    }
    lastAudioMeterSampleMs = now;
    const bool audiblePreview =
        pendingAudibleCursorPreview || timelineEditor->isScrubbingPlayhead() ||
        playbackPipeline->isScrubPlaybackActive();
    pendingAudibleCursorPreview = false;
    playbackPipeline->requestMixedLevelsSampleAsync(timelineEditor->cursorMs(), audiblePreview);
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

void MainWindow::error(const QString &message) const
{
    qWarning() << message;
}

QSize MainWindow::computeMaxSourceVideoResolution() const
{
    int maxW = 0;
    int maxH = 0;
    if (!timelineEditor || !playbackPipeline) {
        error("computeMaxSourceVideoResolution: timelineEditor or playbackPipeline is null");
        return QSize(1, 1);
    }
    const std::shared_ptr<burstcon::ConMediaPool> pool = playbackPipeline->sharedMediaPool();
    if (!pool) {
        error("computeMaxSourceVideoResolution: pool is null");
        return QSize(1, 1);
    }

    QSet<QString> seen;
    auto consider = [&](const QString &path) {
        if (path.isEmpty() || seen.contains(path)) {
            return;
        }
        if (!timelineEditor->hasVideoForPath(path)) {
            return;
        }
        seen.insert(path);
        const std::shared_ptr<burstcon::IBurstcon> media =
            pool->acquire(path, false, false, burstcon::msToBurstIndex(0), true, false);
        if (!media) {
            return;
        }
        const burstcon::BurstOriginInfo info = media->originInfo();
        const int w = info.sourceVideoQuality.width;
        const int h = info.sourceVideoQuality.height;
        if (w > 0 && h > 0) {
            maxW = std::max(maxW, w);
            maxH = std::max(maxH, h);
        }
    };

    for (const auto &clip : timelineEditor->renderClips()) {
        if (clip.isVideo) {
            consider(clip.sourcePath);
        }
    }
    if (sourceArea) {
        for (const QString &p : sourceArea->sourcePaths()) {
            consider(p);
        }
    }

    if (maxW <= 0 || maxH <= 0) {
        error("computeMaxSourceVideoResolution: maxW <= 0 || maxH <= 0");
        return QSize(1, 1);
    }
    return QSize(maxW, maxH);
}

void MainWindow::refreshPreviewResolutionMaxEntry()
{
    if (!previewResolutionCombo || !timelineEditor) {
        return;
    }
    const QSize maxSz = computeMaxSourceVideoResolution();
    const QString label = QStringLiteral("Max (%1×%2)").arg(maxSz.width()).arg(maxSz.height());
    QSignalBlocker blocker(previewResolutionCombo);
    if (previewResolutionCombo->count() >= 4) {
        previewResolutionCombo->setItemText(3, label);
        previewResolutionCombo->setItemData(3, QVariant::fromValue(maxSz));
    } else if (previewResolutionCombo->count() == 3) {
        previewResolutionCombo->addItem(label, QVariant::fromValue(maxSz));
    }
}

void MainWindow::applyRecommendedPreviewCacheQualityForCap()
{
    if (!projectSettings.scaleVideoAtDisplayTime) {
        return;
    }
    Previewer *pv = ui->frame;
    if (!pv) {
        return;
    }
    const qreal dpr = std::max(1.0, static_cast<qreal>(pv->devicePixelRatioF()));
    const int edgePx = std::max(pv->width(), pv->height());
    const int want = static_cast<int>(std::ceil(static_cast<qreal>(edgePx) * dpr * 1.15));
    const int wantClamped = std::max(360, want);

    using CQ = ProjectSettings::CacheQuality;
    static const CQ kOrder[] = {CQ::Quarter16, CQ::Quarter8, CQ::Quarter4, CQ::Half, CQ::Source};
    for (CQ q : kOrder) {
        projectSettings.cacheQuality = q;
        const QSize cr = projectSettings.cacheResolution();
        const int edge = std::max(cr.width(), cr.height());
        if (edge >= wantClamped) {
            return;
        }
    }
    projectSettings.cacheQuality = CQ::Source;
}

void MainWindow::syncPreviewUiFromProjectSettings()
{
    refreshPreviewResolutionMaxEntry();
    if (previewCacheQualityCombo) {
        QSignalBlocker b(previewCacheQualityCombo);
        previewCacheQualityCombo->setCurrentIndex(cacheQualityToComboIndex(projectSettings.cacheQuality));
    }
    if (previewResolutionCombo) {
        QSignalBlocker b(previewResolutionCombo);
        while (previewResolutionCombo->count() > 4) {
            previewResolutionCombo->removeItem(previewResolutionCombo->count() - 1);
        }
        int found = -1;
        for (int i = 0; i < previewResolutionCombo->count(); ++i) {
            const QVariant v = previewResolutionCombo->itemData(i);
            if (!v.canConvert<QSize>()) {
                continue;
            }
            if (v.value<QSize>() == projectSettings.resolution) {
                found = i;
                break;
            }
        }
        if (found >= 0) {
            previewResolutionCombo->setCurrentIndex(found);
        } else {
            const QString label = QStringLiteral("%1×%2")
                                      .arg(projectSettings.resolution.width())
                                      .arg(projectSettings.resolution.height());
            previewResolutionCombo->addItem(label, QVariant::fromValue(projectSettings.resolution));
            previewResolutionCombo->setCurrentIndex(previewResolutionCombo->count() - 1);
        }
    }
    if (previewFpsCombo) {
        QSignalBlocker b(previewFpsCombo);
        previewFpsCombo->setCurrentText(QString::number(projectSettings.fps));
    }
    if (previewSeekLookbackSpin) {
        QSignalBlocker b(previewSeekLookbackSpin);
        previewSeekLookbackSpin->setValue(std::max(0, projectSettings.decodeSeekLookbackMs));
    }
    if (previewSmoothScalingCheck) {
        QSignalBlocker b(previewSmoothScalingCheck);
        previewSmoothScalingCheck->setChecked(projectSettings.previewSmoothScaling);
    }
    if (previewAntialiasingCheck) {
        QSignalBlocker b(previewAntialiasingCheck);
        previewAntialiasingCheck->setChecked(projectSettings.previewAntialiasing);
    }
    if (ui->frame) {
        ui->frame->setSmoothScaling(projectSettings.previewSmoothScaling);
        ui->frame->setAntialiasing(projectSettings.previewAntialiasing);
    }
}

void MainWindow::applyProjectSettings()
{
    if (frontScalerCheck) {
        QSignalBlocker blocker(frontScalerCheck);
        frontScalerCheck->setChecked(projectSettings.scaleVideoAtDisplayTime);
    }
    if (!playbackPipeline) {
        syncPreviewUiFromProjectSettings();
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
    syncPreviewUiFromProjectSettings();
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
    const qint64 prevPerfSampleMs = lastPerfSampleMs;
    const quint64 prevUnusedEvictions = lastResourceUnusedEvictions;
    const quint64 prevResourceMisses = lastResourceCacheMissesSnap;
    const quint64 prevAudioMisses = lastAudioCacheMissesSnap;
    double avcppCallsPerSec = 0.0;
    double avcppOpenPerSec = 0.0;
    double avcppReadPerSec = 0.0;
    double avcppVideoDecodePerSec = 0.0;
    double avcppAudioDecodePerSec = 0.0;
    qint64 perfDeltaMs = 0;
    if (prevPerfSampleMs > 0 && nowMs > prevPerfSampleMs && avcppTotalCalls >= lastAvcppTotalCalls) {
        perfDeltaMs = nowMs - prevPerfSampleMs;
        const quint64 deltaCalls = avcppTotalCalls - lastAvcppTotalCalls;
        avcppCallsPerSec =
            (perfDeltaMs > 0) ? (static_cast<double>(deltaCalls) * 1000.0 / static_cast<double>(perfDeltaMs)) : 0.0;
        const auto perSec = [perfDeltaMs](quint64 deltaValue) {
            return (perfDeltaMs > 0) ? (static_cast<double>(deltaValue) * 1000.0 / static_cast<double>(perfDeltaMs)) : 0.0;
        };
        avcppOpenPerSec = perSec((stats.avcppOpenCalls >= lastAvcppOpenCalls) ? (stats.avcppOpenCalls - lastAvcppOpenCalls) : 0);
        avcppReadPerSec = perSec((stats.avcppPacketReads >= lastAvcppPacketReads) ? (stats.avcppPacketReads - lastAvcppPacketReads) : 0);
        avcppVideoDecodePerSec =
            perSec((stats.avcppVideoDecodeCalls >= lastAvcppVideoDecodeCalls) ? (stats.avcppVideoDecodeCalls - lastAvcppVideoDecodeCalls) : 0);
        avcppAudioDecodePerSec =
            perSec((stats.avcppAudioDecodeCalls >= lastAvcppAudioDecodeCalls) ? (stats.avcppAudioDecodeCalls - lastAvcppAudioDecodeCalls) : 0);
    }
    double noReadDropsPerSec = 0.0;
    double resourceMissesPerSec = 0.0;
    double audioMissesPerSec = 0.0;
    const qint64 diagDeltaMs =
        (prevPerfSampleMs > 0 && nowMs > prevPerfSampleMs) ? (nowMs - prevPerfSampleMs) : 0;
    if (performanceDiagnosticsWindow && diagDeltaMs > 0) {
        const auto perSecDiag = [diagDeltaMs](quint64 deltaValue) {
            return (diagDeltaMs > 0) ? (static_cast<double>(deltaValue) * 1000.0 / static_cast<double>(diagDeltaMs)) : 0.0;
        };
        const quint64 dEv = (stats.resourceCacheEvictions >= prevUnusedEvictions)
            ? (stats.resourceCacheEvictions - prevUnusedEvictions)
            : 0;
        const quint64 dMiss = (stats.resourceCacheMisses >= prevResourceMisses)
            ? (stats.resourceCacheMisses - prevResourceMisses)
            : 0;
        const quint64 dAudMiss = (stats.audioCacheMisses >= prevAudioMisses)
            ? (stats.audioCacheMisses - prevAudioMisses)
            : 0;
        noReadDropsPerSec = perSecDiag(dEv);
        resourceMissesPerSec = perSecDiag(dMiss);
        audioMissesPerSec = perSecDiag(dAudMiss);
        constexpr double kPoolMaxEntries = 384.0;
        const double cacheOccupancyPct =
            std::min(100.0, (static_cast<double>(stats.resourceCacheEntries) * 100.0) / kPoolMaxEntries);
        performanceDiagnosticsWindow->ingestDiagnosticsSamples(
            noReadDropsPerSec, resourceMissesPerSec, cacheOccupancyPct, audioMissesPerSec);
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
        PlaybackPipeline::Clip c;
        c.sourcePath = clip.sourcePath;
        c.startMs = clip.startMs;
        c.durationMs = clip.durationMs;
        c.sourceInMs = clip.sourceInMs;
        c.trackIndex = clip.trackIndex;
        c.videoFxChain = clip.videoFxChain;
        c.audioFxChain = clip.audioFxChain;
        c.audioLinearGain = clip.audioLinearGain;
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
    const auto timelineVideoSliding = playbackPipeline->timelineVideoSlidingSegments(viewStart, viewEnd, -1);
    const auto timelineVideoHead = playbackPipeline->timelineVideoHeadSegments(viewStart, viewEnd);
    const auto timelineAudioSliding = playbackPipeline->timelineAudioSlidingSegments(viewStart, viewEnd, -1);
    if (!timelineEditor->isEditingTimelineContent()) {
        timelineEditor->setCacheCoverage(clipVideoSliding,
                                         clipVideoHead,
                                         timelineVideoSliding,
                                         timelineVideoHead,
                                         timelineAudioSliding,
                                         clipAudio);
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
        QString("Resource cache %1x%2 (%3) | decoded: %4 | entries: %5 | no-read drops: %6 | used/limit: %7 / %8 (max %9)")
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

    if (performanceDiagnosticsWindow && perfDeltaMs > 0) {
        const quint64 duThisTick = (stats.resourceCacheEvictions >= prevUnusedEvictions)
            ? (stats.resourceCacheEvictions - prevUnusedEvictions)
            : 0;
        if (duThisTick > 0) {
            performanceDiagnosticsWindow->appendProblem(
                tr("Resource video cache dropped %1 frame(s) before any read (LRU cap, overwrite, or invalidate).")
                    .arg(duThisTick));
        }
    }

    lastResourceUnusedEvictions = stats.resourceCacheEvictions;
    lastResourceCacheMissesSnap = stats.resourceCacheMisses;
    lastAudioCacheMissesSnap = stats.audioCacheMisses;
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
    settingsObject.insert(QStringLiteral("frontScaler"), projectSettings.scaleVideoAtDisplayTime);
    settingsObject.insert(QStringLiteral("scaleVideoAtDisplayTime"), projectSettings.scaleVideoAtDisplayTime);
    settingsObject.insert("previewSmoothScaling", projectSettings.previewSmoothScaling);
    settingsObject.insert("previewAntialiasing", projectSettings.previewAntialiasing);
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
        if (settingsObject.contains(QStringLiteral("frontScaler"))) {
            projectSettings.scaleVideoAtDisplayTime =
                settingsObject.value(QStringLiteral("frontScaler")).toBool(true);
        } else {
            projectSettings.scaleVideoAtDisplayTime =
                settingsObject.value(QStringLiteral("scaleVideoAtDisplayTime")).toBool(true);
        }
        projectSettings.previewSmoothScaling =
            settingsObject.value("previewSmoothScaling").toBool(false);
        projectSettings.previewAntialiasing =
            settingsObject.value("previewAntialiasing").toBool(false);
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

void placeMorpherWindowTitlebarUnderCursor(QWidget *w)
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

void MainWindow::openClipFxChainWindow(int clipId, ClipFxChainWindow::MediaKind kind, const QString &focusHint)
{
    if (!timelineEditor || clipId < 0) {
        return;
    }
    const bool wantVideo = (kind == ClipFxChainWindow::MediaKind::Video);
    if (!timelineEditor->clipMatchesFxTarget(clipId, wantVideo)) {
        return;
    }
    const qint64 key = (static_cast<qint64>(clipId) << 1) | (wantVideo ? 0LL : 1LL);
    if (ClipFxChainWindow *existing = clipFxWindows.value(key)) {
        existing->raise();
        existing->activateWindow();
        return;
    }
    auto *win = new ClipFxChainWindow(this, timelineEditor, clipId, kind, focusHint, projectSettings.resolution);
    clipFxWindows.insert(key, win);
    connect(win, &QObject::destroyed, this, [this, key]() { clipFxWindows.remove(key); });
    win->show();
    QPointer<ClipFxChainWindow> winGuard(win);
    QTimer::singleShot(0, win, [winGuard]() {
        if (winGuard) {
            placeMorpherWindowTitlebarUnderCursor(winGuard.data());
        }
    });
}

void MainWindow::pruneClipFxWindows()
{
    if (!timelineEditor) {
        return;
    }
    const QList<qint64> keys = clipFxWindows.keys();
    for (const qint64 key : keys) {
        const int clipId = static_cast<int>(key >> 1);
        const bool wantVideo = (key & 1) == 0;
        if (!timelineEditor->clipMatchesFxTarget(clipId, wantVideo)) {
            if (ClipFxChainWindow *w = clipFxWindows.value(key)) {
                w->close();
            }
        }
    }
}

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

void MainWindow::openPerformanceDiagnosticsWindow()
{
    if (performanceDiagnosticsWindow) {
        performanceDiagnosticsWindow->show();
        performanceDiagnosticsWindow->raise();
        performanceDiagnosticsWindow->activateWindow();
        return;
    }
    auto *w = new PerformanceDiagnosticsWindow(this);
    performanceDiagnosticsWindow = w;
    burstcon::setBurstconDiagnosticHandler([this](const QString &msg) {
        QPointer<PerformanceDiagnosticsWindow> wg = performanceDiagnosticsWindow;
        QMetaObject::invokeMethod(
            this,
            [wg, msg]() {
                if (wg) {
                    wg->appendProblem(msg);
                }
            },
            Qt::QueuedConnection);
    });
    connect(w, &QObject::destroyed, this, [this]() {
        burstcon::setBurstconDiagnosticHandler(nullptr);
        performanceDiagnosticsWindow = nullptr;
    });
    w->show();
    QPointer<PerformanceDiagnosticsWindow> guard(w);
    QTimer::singleShot(0, w, [guard]() {
        if (guard) {
            placeMorpherWindowTitlebarUnderCursor(guard.data());
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

void MainWindow::tickMediaSourceAvailability()
{
    if (!timelineEditor) {
        return;
    }
    QSet<QString> activePaths;
    for (const auto &c : timelineEditor->renderClips()) {
        if (!c.sourcePath.isEmpty()) {
            activePaths.insert(c.sourcePath);
        }
    }
    for (auto it = mediaPathPreviouslyReachable.begin(); it != mediaPathPreviouslyReachable.end();) {
        if (!activePaths.contains(it.key())) {
            it = mediaPathPreviouslyReachable.erase(it);
        } else {
            ++it;
        }
    }
    for (const QString &p : activePaths) {
        const bool ok = QFileInfo::exists(p);
        const auto prevIt = mediaPathPreviouslyReachable.constFind(p);
        if (prevIt == mediaPathPreviouslyReachable.cend()) {
            mediaPathPreviouslyReachable.insert(p, ok);
            continue;
        }
        if (prevIt.value() != ok) {
            mediaPathPreviouslyReachable.insert(p, ok);
            if (playbackPipeline) {
                playbackPipeline->sharedMediaPool()->invalidatePath(p);
                if (stripMediaPool) {
                    stripMediaPool->invalidatePath(p);
                }
            }
            timelineEditor->invalidateSourcePath(p);
            schedulePlaybackSync();
            requestCacheCoverageUiRefresh();
        }
    }
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
        const QList<ClipFxChainWindow *> fxs = clipFxWindows.values();
        for (ClipFxChainWindow *w : fxs) {
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

