#include "performancewidget.h"

#include "multilinegraphwidget.h"

#include <cmath>

#include <QGuiApplication>
#include <QHBoxLayout>
#include <QLabel>
#include <QScreen>
#include <QSizePolicy>
#include <QToolButton>
#include <QVBoxLayout>

PerformanceWidget::PerformanceWidget(QWidget *parent)
    : QWidget(parent)
{
    const QScreen *scr = QGuiApplication::primaryScreen();
    const qreal dpiScale = scr ? std::clamp(scr->logicalDotsPerInch() / 96.0, 1.0, 2.4) : 1.0;
    const int m = std::max(6, static_cast<int>(std::lround(8.0 * dpiScale)));

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(m, m, m, m);
    layout->setSpacing(std::max(4, static_cast<int>(std::lround(5.0 * dpiScale))));
    setMinimumHeight(460);
    setStyleSheet(QStringLiteral(
        "PerformanceWidget {\n"
        "  background-color:#1b1d20;\n"
        "  border-top:1px solid #343a42;\n"
        "}\n"
        "QLabel { color:#cfd6df; background-color:transparent; }"));

    auto *titleRow = new QHBoxLayout();
    titleRow->setSpacing(8);
    auto *title = new QLabel(QStringLiteral("Performance / Utility"), this);
    title->setStyleSheet(QStringLiteral("font-weight:600; color:#cfd6df; background:transparent;"));
    auto *diagBtn = new QToolButton(this);
    diagBtn->setText(tr("Diagnostics…"));
    diagBtn->setToolTip(tr("Open performance diagnostics (cache pressure, problem log)"));
    diagBtn->setAutoRaise(true);
    connect(diagBtn, &QToolButton::clicked, this, &PerformanceWidget::diagnosticsWindowRequested);
    titleRow->addWidget(title, 0, Qt::AlignLeft | Qt::AlignVCenter);
    titleRow->addStretch(1);
    titleRow->addWidget(diagBtn, 0, Qt::AlignRight | Qt::AlignVCenter);

    composeFpsLabel = new QLabel("Compose FPS: -", this);
    cacheStatsLabel = new QLabel("Cache: -", this);
    cacheResolutionLabel = new QLabel("Cache Resolution: -", this);
    projectInfoLabel = new QLabel("Project: -", this);
    qualityGraph = new MultiLineGraphWidget(
        "Quality Metrics",
        {"FPS", "CacheHit", "FinalHit", "AudioHit"},
        {QColor(78, 200, 110), QColor(74, 166, 255), QColor(250, 190, 76), QColor(181, 132, 255)},
        {120.0, 100.0, 100.0, 100.0},
        MultiLineAxisStyle::NormalizedPercentGrid,
        180,
        this);
    activityGraph = new MultiLineGraphWidget(
        "avcpp Activity",
        {"Total/s", "Open/s", "Read/s", "VDec/s", "ADec/s"},
        {QColor(255, 145, 80), QColor(255, 205, 105), QColor(244, 99, 99), QColor(95, 209, 155), QColor(130, 173, 255)},
        {5000.0, 500.0, 5000.0, 1500.0, 1500.0},
        MultiLineAxisStyle::NormalizedPercentGrid,
        180,
        this);
    qualityGraph->setMinimumHeight(170);
    activityGraph->setMinimumHeight(170);

    layout->addLayout(titleRow);
    layout->addWidget(composeFpsLabel);
    layout->addWidget(cacheStatsLabel);
    layout->addWidget(cacheResolutionLabel);
    layout->addWidget(projectInfoLabel);
    layout->addWidget(qualityGraph, 3);
    layout->addWidget(activityGraph, 3);
}

void PerformanceWidget::setMetrics(const QString &composeFps,
                                   const QString &cacheStats,
                                   const QString &cacheResolution,
                                   const QString &projectInfo,
                                   double fpsValue,
                                   double cacheHitPct,
                                   double finalCacheHitPct,
                                   double audioCacheHitPct,
                                   double avcppCallsPerSecond,
                                   double avcppOpenPerSec,
                                   double avcppReadPerSec,
                                   double avcppVideoDecodePerSec,
                                   double avcppAudioDecodePerSec)
{
    composeFpsLabel->setText(composeFps);
    cacheStatsLabel->setText(cacheStats);
    cacheResolutionLabel->setText(cacheResolution);
    projectInfoLabel->setText(projectInfo);
    qualityGraph->addSample({fpsValue, cacheHitPct, finalCacheHitPct, audioCacheHitPct});
    activityGraph->addSample({avcppCallsPerSecond,
                              avcppOpenPerSec,
                              avcppReadPerSec,
                              avcppVideoDecodePerSec,
                              avcppAudioDecodePerSec});
}
