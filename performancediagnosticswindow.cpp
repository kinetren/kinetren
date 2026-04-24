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
    setMinimumSize(520, 420);
    resize(640, 480);
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
    auto *title = new QLabel(tr("Problems & cache pressure"), this);
    title->setObjectName(QStringLiteral("PerfDiagTitle"));
    auto *clearBtn = new QPushButton(tr("Clear log"), this);
    connect(clearBtn, &QPushButton::clicked, this, [this]() { clearProblems(); });
    titleRow->addWidget(title, 1);
    titleRow->addWidget(clearBtn, 0, Qt::AlignRight);

    logBrowser = new QTextBrowser(this);
    logBrowser->setReadOnly(true);
    logBrowser->setOpenExternalLinks(false);
    logBrowser->document()->setMaximumBlockCount(400);
    logBrowser->setMinimumHeight(140);

    graph = new MultiLineGraphWidget(
        tr("Cache / decode (rolling)"),
        {tr("Unused evict/s"), tr("VDec/s"), tr("Res.hit %"), tr("Cache fill %")},
        {QColor(255, 120, 120), QColor(95, 209, 155), QColor(74, 166, 255), QColor(250, 190, 76)},
        {40.0, 1200.0, 100.0, 100.0},
        MultiLineAxisStyle::NormalizedPercentGrid,
        180,
        this);
    graph->setMinimumHeight(200);

    auto *lay = new QVBoxLayout(this);
    lay->setContentsMargins(12, 10, 12, 10);
    lay->setSpacing(8);
    lay->addLayout(titleRow);
    lay->addWidget(logBrowser, 1);
    lay->addWidget(graph, 2);

    appendProblem(tr("Tip: “Unused evict” counts ConCache video frames removed by LRU or overwritten "
                     "before any read — heavy prefetch vs tight limits shows up here."));
}

void PerformanceDiagnosticsWindow::ingestGraphSample(const double unusedEvictPerSec,
                                                     const double videoDecodePerSec,
                                                     const double resourceHitPct,
                                                     const double cacheEntriesPercent)
{
    if (!graph) {
        return;
    }
    graph->addSample({unusedEvictPerSec, videoDecodePerSec, resourceHitPct, cacheEntriesPercent});
}

void PerformanceDiagnosticsWindow::appendProblem(const QString &line)
{
    if (!logBrowser) {
        return;
    }
    const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("HH:mm:ss"));
    logBrowser->append(QStringLiteral("[%1] %2").arg(stamp, line));
}

void PerformanceDiagnosticsWindow::clearProblems()
{
    if (logBrowser) {
        logBrowser->clear();
    }
}
