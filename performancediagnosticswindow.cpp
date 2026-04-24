#include "performancediagnosticswindow.h"

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
    setMinimumSize(520, 360);
    resize(680, 520);
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
    logBrowser->setMinimumHeight(220);

    auto *lay = new QVBoxLayout(this);
    lay->setContentsMargins(12, 10, 12, 10);
    lay->setSpacing(8);
    lay->addLayout(titleRow);
    lay->addWidget(logBrowser, 1);

    appendProblem(tr("Decode / seek / I/O issues from burstcon and the timeline strip appear here. "
                     "Open this window before reproducing a problem so the handler is armed."));
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
