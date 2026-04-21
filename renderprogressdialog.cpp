#include "renderprogressdialog.h"

#include <QFont>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QVBoxLayout>

#include <algorithm>

RenderProgressDialog::RenderProgressDialog(QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle("Render to MP4");
    setModal(true);
    resize(920, 640);
    setMinimumSize(720, 520);

    m_cancelFlag = std::make_shared<std::atomic<bool>>(false);
    m_pauseFlag = std::make_shared<std::atomic<bool>>(false);

    m_status = new QLabel(this);
    m_status->setWordWrap(true);
    m_status->setTextFormat(Qt::RichText);
    m_status->setOpenExternalLinks(false);

    m_bar = new QProgressBar(this);
    m_bar->setRange(0, 1000);
    m_bar->setValue(0);

    m_log = new QPlainTextEdit(this);
    m_log->setReadOnly(true);
    m_log->setMaximumBlockCount(800);
    m_log->setFont(QFont(QStringLiteral("monospace"), 10));

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
    lay->addWidget(m_status);
    lay->addWidget(m_bar);
    lay->addWidget(m_log, 1);
    lay->addLayout(btnRow);
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

void RenderProgressDialog::setStatusHtml(const QString &html)
{
    if (m_status) {
        m_status->setText(html);
    }
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
