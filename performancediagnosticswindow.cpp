#include "performancediagnosticswindow.h"

#include "multilinegraphwidget.h"

#include <QApplication>
#include <QDateTime>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QTextBrowser>
#include <QVBoxLayout>

PerformanceDiagnosticsWindow::PerformanceDiagnosticsWindow(QWidget *parent)
    : QWidget(parent)
{
    setObjectName(QStringLiteral("PerformanceDiagnosticsWindow"));
    setWindowFlags(Qt::Tool | Qt::Window | Qt::WindowCloseButtonHint | Qt::WindowMinimizeButtonHint |
                   Qt::WindowMaximizeButtonHint);
    setAttribute(Qt::WA_DeleteOnClose, true);
    setMinimumSize(520, 480);
    resize(720, 560);
    setWindowTitle(tr("Performance diagnostics"));
    if (QWidget *pw = parentWidget()) {
        setFont(pw->font());
    } else {
        setFont(QApplication::font());
    }

    setStyleSheet(QStringLiteral(
        "#PerformanceDiagnosticsWindow { background: #0d1117; color: #e6edf3; }\n"
        "QLabel { color: #c9d1d9; }\n"
        "QLabel#PerfDiagTitle { color: #f0f3f6; font-weight: 600; }\n"
        "QTextBrowser {\n"
        "  background: #161b22; color: #e6edf3; border: 1px solid #30363d; border-radius: 6px;\n"
        "  selection-background-color: #1f6feb;\n"
        "}\n"
        "QPushButton {\n"
        "  background: #21262d; color: #e6edf3; border: 1px solid #30363d; border-radius: 6px;\n"
        "  padding: 6px 14px;\n"
        "}\n"
        "QPushButton:hover { background: #30363d; border-color: #484f58; }\n"
        "QPushButton:pressed { background: #161b22; }\n"));

    auto *titleRow = new QHBoxLayout();
    auto *title = new QLabel(tr("Diagnostics log"), this);
    title->setObjectName(QStringLiteral("PerfDiagTitle"));
    auto *clearBtn = new QPushButton(tr("Clear log"), this);
    connect(clearBtn, &QPushButton::clicked, this, [this]() { clearProblems(); });
    titleRow->addWidget(title, 1);
    titleRow->addWidget(clearBtn, 0, Qt::AlignRight);

    logBrowser = new QTextBrowser(this);
    logBrowser->setReadOnly(true);
    logBrowser->setOpenExternalLinks(false);
    logBrowser->document()->setMaximumBlockCount(800);
    logBrowser->setMinimumHeight(160);

    churnGraph = new MultiLineGraphWidget(
        tr("Cache churn (not on main panel)"),
        {tr("No-read drops/s"),
         tr("Res.cache miss/s"),
         tr("Cache occupancy %"),
         tr("Audio cache miss/s")},
        {QColor(255, 120, 120), QColor(255, 180, 70), QColor(120, 200, 255), QColor(200, 140, 255)},
        {80.0, 2000.0, 100.0, 400.0},
        MultiLineAxisStyle::NormalizedPercentGrid,
        180,
        this);
    churnGraph->setMinimumHeight(220);

    auto *lay = new QVBoxLayout(this);
    lay->setContentsMargins(12, 10, 12, 10);
    lay->setSpacing(8);
    lay->addLayout(titleRow);
    lay->addWidget(logBrowser, 2);
    lay->addWidget(churnGraph, 3);

    appendProblem(tr("Main window graphs: Quality (FPS, hit %…) and avcpp Activity (read/decode/s…). "
                     "Here: drop rate, ConCache miss rate, occupancy, audio-cache miss rate."));
}

void PerformanceDiagnosticsWindow::ingestDiagnosticsSamples(const double noReadDropsPerSec,
                                                              const double resourceCacheMissesPerSec,
                                                              const double cacheOccupancyPct,
                                                              const double audioCacheMissesPerSec)
{
    if (!churnGraph) {
        return;
    }
    churnGraph->addSample({noReadDropsPerSec, resourceCacheMissesPerSec, cacheOccupancyPct, audioCacheMissesPerSec});
}

void PerformanceDiagnosticsWindow::appendProblem(const QString &line)
{
    if (!logBrowser) {
        return;
    }
    const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("HH:mm:ss.zzz"));
    logBrowser->append(QStringLiteral("[%1] %2").arg(stamp, line));
}

void PerformanceDiagnosticsWindow::clearProblems()
{
    if (logBrowser) {
        logBrowser->clear();
    }
}
