#include "renderprogressdialog.h"
#include "speedchartwidget.h"

#include <QFont>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QTextCursor>
#include <QVBoxLayout>

#include <algorithm>
#include <cmath>

namespace {
QFont monoFont(int pointSize = 10)
{
    QFont f(QStringLiteral("monospace"), pointSize);
    f.setStyleHint(QFont::TypeWriter);
    return f;
}

QLabel *makeValueLabel(QWidget *parent)
{
    auto *l = new QLabel(parent);
    l->setFont(monoFont(10));
    l->setTextInteractionFlags(Qt::TextSelectableByMouse);
    l->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    l->setMinimumWidth(280);
    l->setStyleSheet(QStringLiteral("QLabel { padding: 2px 0; }"));
    return l;
}
} // namespace

RenderProgressDialog::RenderProgressDialog(QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle("Render to MP4");
    setModal(true);
    resize(920, 640);
    setMinimumSize(720, 520);
    setStyleSheet(QStringLiteral(
        "QDialog { background-color: #1c1e22; color: #dce4ef; }"
        "QPlainTextEdit { background-color: #17191c; color: #d7dee8; border: 1px solid #2f3540; border-radius: 4px; }"
        "QProgressBar { border: 1px solid #2f3540; border-radius: 4px; background: #131519; min-height: 18px; }"
        "QProgressBar::chunk { background-color: #3a7eb5; }"));

    m_cancelFlag = std::make_shared<std::atomic<bool>>(false);
    m_pauseFlag = std::make_shared<std::atomic<bool>>(false);

    auto *metricsBox = new QWidget(this);
    auto *form = new QFormLayout(metricsBox);
    form->setContentsMargins(0, 0, 0, 0);
    form->setSpacing(6);
    form->setHorizontalSpacing(16);
    form->setFieldGrowthPolicy(QFormLayout::FieldsStayAtSizeHint);
    form->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);

    const QString labelStyle = QStringLiteral("QLabel { color: #aeb6c2; min-width: 7em; }");
    auto makeKey = [labelStyle](const QString &text) {
        auto *lb = new QLabel(text);
        lb->setStyleSheet(labelStyle);
        lb->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
        return lb;
    };

    m_valPhase = makeValueLabel(metricsBox);
    m_valProgress = makeValueLabel(metricsBox);
    m_valElapsed = makeValueLabel(metricsBox);
    m_valRemaining = makeValueLabel(metricsBox);
    m_valOutput = makeValueLabel(metricsBox);

    form->addRow(makeKey(tr("Phase")), m_valPhase);
    form->addRow(makeKey(tr("Progress")), m_valProgress);
    form->addRow(makeKey(tr("Elapsed")), m_valElapsed);
    form->addRow(makeKey(tr("Remaining")), m_valRemaining);
    form->addRow(makeKey(tr("Output len")), m_valOutput);

    m_speedStats = new QLabel(this);
    m_speedStats->setFont(monoFont(10));
    m_speedStats->setTextInteractionFlags(Qt::TextSelectableByMouse);
    m_speedStats->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    m_speedStats->setMinimumHeight(22);
    m_speedStats->setStyleSheet(QStringLiteral("QLabel { padding: 4px 0; color: #dce4ef; }"));
    m_speedStats->setText(tr("Speed:  min —   avg —   cur —   max —"));

    m_chart = new SpeedChartWidget(this);

    m_bar = new QProgressBar(this);
    m_bar->setRange(0, 1000);
    m_bar->setValue(0);
    m_bar->setTextVisible(true);
    m_bar->setFormat(QStringLiteral("%p%"));
    m_bar->setMinimumHeight(22);

    m_log = new QPlainTextEdit(this);
    m_log->setReadOnly(true);
    m_log->setMaximumBlockCount(800);
    m_log->setFont(monoFont(10));

    m_pauseBtn = new QPushButton(tr("Pause"), this);
    m_cancelBtn = new QPushButton(tr("Cancel"), this);
    m_okBtn = new QPushButton(tr("OK"), this);
    m_okBtn->setVisible(false);
    m_okBtn->setDefault(true);

    connect(m_pauseBtn, &QPushButton::clicked, this, &RenderProgressDialog::onPauseClicked);
    connect(m_cancelBtn, &QPushButton::clicked, this, &RenderProgressDialog::onCancelClicked);
    connect(m_okBtn, &QPushButton::clicked, this, &QDialog::accept);

    auto *btnRow = new QHBoxLayout();
    btnRow->addWidget(m_pauseBtn);
    btnRow->addWidget(m_cancelBtn);
    btnRow->addStretch(1);
    btnRow->addWidget(m_okBtn);

    auto *lay = new QVBoxLayout(this);
    lay->setSpacing(8);
    lay->addWidget(metricsBox);
    lay->addWidget(m_speedStats);
    lay->addWidget(m_chart);
    lay->addWidget(m_bar);
    lay->addWidget(m_log, 1);
    lay->addLayout(btnRow);

    m_valPhase->setText(QStringLiteral("—"));
    m_valProgress->setText(QStringLiteral("—"));
    m_valElapsed->setText(QStringLiteral("—"));
    m_valRemaining->setText(QStringLiteral("—"));
    m_valOutput->setText(QStringLiteral("—"));
}

void RenderProgressDialog::appendLog(const QString &line)
{
    if (m_log) {
        m_log->appendPlainText(line);
        QTextCursor c = m_log->textCursor();
        c.movePosition(QTextCursor::End);
        m_log->setTextCursor(c);
    }
}

void RenderProgressDialog::setProgress(int value, int maximum)
{
    if (m_bar) {
        m_bar->setRange(0, std::max(1, maximum));
        m_bar->setValue(std::clamp(value, 0, std::max(1, maximum)));
    }
}

void RenderProgressDialog::setProgressView(const QString &phase,
                                           int done,
                                           int total,
                                           const QString &elapsed,
                                           const QString &remaining,
                                           const QString &outputLen,
                                           double speedRealtimeFactor)
{
    if (m_valPhase) {
        m_valPhase->setText(phase);
    }
    if (m_valProgress) {
        m_valProgress->setText(QStringLiteral("%1 / %2").arg(done).arg(total));
    }
    if (m_valElapsed) {
        m_valElapsed->setText(elapsed);
    }
    if (m_valRemaining) {
        m_valRemaining->setText(remaining);
    }
    if (m_valOutput) {
        m_valOutput->setText(outputLen);
    }

    if (std::isfinite(speedRealtimeFactor) && speedRealtimeFactor > 1e-6) {
        if (!m_haveSpeedStats) {
            m_speedMin = m_speedMax = speedRealtimeFactor;
            m_haveSpeedStats = true;
        } else {
            m_speedMin = std::min(m_speedMin, speedRealtimeFactor);
            m_speedMax = std::max(m_speedMax, speedRealtimeFactor);
        }
        m_speedSum += speedRealtimeFactor;
        ++m_speedCount;
        m_speedCurrent = speedRealtimeFactor;
        if (m_chart) {
            m_chart->addSample(speedRealtimeFactor);
        }
    }
    updateSpeedStatsLine();
}

void RenderProgressDialog::updateSpeedStatsLine()
{
    if (!m_speedStats) {
        return;
    }
    if (m_speedCount <= 0 || !m_haveSpeedStats) {
        m_speedStats->setText(tr("Speed:  min —    avg —    cur —    max —"));
        return;
    }
    const double avg = m_speedSum / static_cast<double>(m_speedCount);
    const QChar sp = QLatin1Char(' ');
    const QString line = QStringLiteral("Speed:  min %1×   avg %2×   cur %3×   max %4×")
                             .arg(m_speedMin, 7, 'f', 2, sp)
                             .arg(avg, 7, 'f', 2, sp)
                             .arg(m_speedCurrent, 7, 'f', 2, sp)
                             .arg(m_speedMax, 7, 'f', 2, sp);
    m_speedStats->setText(line);
}

void RenderProgressDialog::setRunning(bool running)
{
    if (m_pauseBtn) {
        m_pauseBtn->setEnabled(running);
    }
    if (m_cancelBtn) {
        m_cancelBtn->setEnabled(running);
    }
}

void RenderProgressDialog::setFinished(bool ok, const QString &summary)
{
    setRunning(false);
    if (m_bar) {
        m_bar->setValue(m_bar->maximum());
    }
    if (m_pauseBtn) {
        m_pauseBtn->setVisible(false);
    }
    if (m_cancelBtn) {
        m_cancelBtn->setVisible(false);
    }
    if (m_okBtn) {
        m_okBtn->setVisible(true);
        m_okBtn->setFocus();
    }
    appendLog(QString());
    appendLog(ok ? tr("Finished successfully.") : tr("Finished with errors or cancellation."));
    if (!summary.isEmpty()) {
        appendLog(summary);
    }
}

void RenderProgressDialog::onPauseClicked()
{
    const bool nowPaused = !m_pauseFlag->load();
    m_pauseFlag->store(nowPaused);
    m_pauseBtn->setText(nowPaused ? tr("Resume") : tr("Pause"));
}

void RenderProgressDialog::onCancelClicked()
{
    m_cancelFlag->store(true);
    appendLog(tr("Cancel requested…"));
    m_cancelBtn->setEnabled(false);
}
