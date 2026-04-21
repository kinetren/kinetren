#include "renderprogressdialog.h"
#include "speedchartwidget.h"
#include "projectsettings.h"

#include <QDialog>
#include <QFont>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QStyle>
#include <QTextCursor>
#include <QToolButton>
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
    l->setFont(monoFont(11));
    l->setTextInteractionFlags(Qt::TextSelectableByMouse);
    l->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    l->setMinimumWidth(300);
    l->setStyleSheet(QStringLiteral("QLabel { padding: 2px 0; }"));
    return l;
}

QSize evenRenderSize(const QSize &in)
{
    int w = std::max(2, in.width());
    int h = std::max(2, in.height());
    if ((w % 2) != 0) {
        ++w;
    }
    if ((h % 2) != 0) {
        ++h;
    }
    return QSize(w, h);
}
} // namespace

RenderProgressDialog::RenderProgressDialog(QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle("Render to MP4");
    setModal(true);
    resize(1180, 720);
    setMinimumSize(980, 600);
    setStyleSheet(QStringLiteral(
        "QDialog { background-color: #1c1e22; color: #dce4ef; }"
        "QPlainTextEdit { background-color: #17191c; color: #d7dee8; border: 1px solid #2f3540; border-radius: 4px; }"
        "QProgressBar { border: 1px solid #2f3540; border-radius: 4px; background: #131519; min-height: 22px; "
        "text-align: center; }"
        "QProgressBar::chunk { background-color: #3a7eb5; }"
        "QToolButton { background: #2a3038; border: 1px solid #3d4652; border-radius: 4px; padding: 4px; }"
        "QToolButton:hover { background: #343c48; }"));

    m_cancelFlag = std::make_shared<std::atomic<bool>>(false);
    m_pauseFlag = std::make_shared<std::atomic<bool>>(false);

    const QString labelStyle = QStringLiteral("QLabel { color: #aeb6c2; min-width: 7em; }");
    auto makeKey = [labelStyle](const QString &text) {
        auto *lb = new QLabel(text);
        lb->setStyleSheet(labelStyle);
        lb->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
        return lb;
    };

    auto buildForm = [&](QWidget *box) {
        auto *form = new QFormLayout(box);
        form->setContentsMargins(0, 0, 0, 0);
        form->setSpacing(3);
        form->setHorizontalSpacing(12);
        form->setFieldGrowthPolicy(QFormLayout::FieldsStayAtSizeHint);
        form->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
        return form;
    };

    auto *outBox = new QWidget(this);
    auto *outForm = buildForm(outBox);
    m_outResolution = makeValueLabel(outBox);
    m_outFps = makeValueLabel(outBox);
    m_outVideo = makeValueLabel(outBox);
    m_outAudio = makeValueLabel(outBox);
    m_outMux = makeValueLabel(outBox);
    outForm->addRow(makeKey(tr("Resolution")), m_outResolution);
    outForm->addRow(makeKey(tr("Frame rate")), m_outFps);
    outForm->addRow(makeKey(tr("Video")), m_outVideo);
    outForm->addRow(makeKey(tr("Audio")), m_outAudio);
    outForm->addRow(makeKey(tr("Mux")), m_outMux);

    auto *runBox = new QWidget(this);
    auto *form = buildForm(runBox);
    m_valPhase = makeValueLabel(runBox);
    m_valProgress = makeValueLabel(runBox);
    m_valElapsed = makeValueLabel(runBox);
    m_valRemaining = makeValueLabel(runBox);
    m_valOutput = makeValueLabel(runBox);
    form->addRow(makeKey(tr("Phase")), m_valPhase);
    form->addRow(makeKey(tr("Progress")), m_valProgress);
    form->addRow(makeKey(tr("Elapsed")), m_valElapsed);
    form->addRow(makeKey(tr("Remaining")), m_valRemaining);
    form->addRow(makeKey(tr("Output len")), m_valOutput);

    auto *metricsRow = new QHBoxLayout();
    metricsRow->setSpacing(20);
    metricsRow->addWidget(outBox, 1);
    metricsRow->addWidget(runBox, 1);

    auto *metricsWrap = new QWidget(this);
    metricsWrap->setLayout(metricsRow);

    m_speedStats = new QLabel(this);
    m_speedStats->setFont(monoFont(11));
    m_speedStats->setTextInteractionFlags(Qt::TextSelectableByMouse);
    m_speedStats->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    m_speedStats->setMinimumHeight(22);
    m_speedStats->setStyleSheet(QStringLiteral("QLabel { padding: 2px 0; color: #dce4ef; }"));
    m_speedStats->setText(tr("Speed:  min —   avg —   cur —   max —"));

    m_chart = new SpeedChartWidget(this);

    m_bar = new QProgressBar(this);
    m_bar->setRange(0, 1000);
    m_bar->setValue(0);
    m_bar->setTextVisible(true);
    m_bar->setFormat(QStringLiteral("%p%"));
    m_bar->setMinimumHeight(22);
    m_bar->setAlignment(Qt::AlignCenter);

    m_logBtn = new QToolButton(this);
    m_logBtn->setIcon(style()->standardIcon(QStyle::SP_MessageBoxInformation));
    m_logBtn->setIconSize(QSize(22, 22));
    m_logBtn->setFixedSize(36, 32);
    m_logBtn->setAutoRaise(false);
    m_logBtn->setFocusPolicy(Qt::NoFocus);
    m_logBtn->setToolTip(tr("Show log"));
    connect(m_logBtn, &QToolButton::clicked, this, &RenderProgressDialog::onLogWindowClicked);

    m_log = new QPlainTextEdit(this);
    m_log->setReadOnly(true);
    m_log->setMaximumBlockCount(800);
    m_log->setFont(monoFont(10));
    m_log->hide();

    m_pauseBtn = new QPushButton(tr("Pause"), this);
    m_cancelBtn = new QPushButton(tr("Cancel"), this);
    m_okBtn = new QPushButton(tr("OK"), this);
    m_okBtn->setVisible(false);
    m_okBtn->setDefault(true);

    connect(m_pauseBtn, &QPushButton::clicked, this, &RenderProgressDialog::onPauseClicked);
    connect(m_cancelBtn, &QPushButton::clicked, this, &RenderProgressDialog::onCancelClicked);
    connect(m_okBtn, &QPushButton::clicked, this, &QDialog::accept);

    auto *barRow = new QHBoxLayout();
    barRow->setSpacing(0);
    barRow->addWidget(m_bar, 1);

    auto *btnRow = new QHBoxLayout();
    btnRow->setContentsMargins(0, 2, 0, 0);
    btnRow->setSpacing(8);
    btnRow->addStretch(1);
    btnRow->addWidget(m_pauseBtn);
    btnRow->addWidget(m_cancelBtn);
    btnRow->addWidget(m_logBtn, 0, Qt::AlignVCenter);
    btnRow->addWidget(m_okBtn);

    auto *lay = new QVBoxLayout(this);
    lay->setContentsMargins(8, 6, 8, 6);
    lay->setSpacing(4);
    lay->addWidget(metricsWrap);
    lay->addWidget(m_speedStats);
    lay->addWidget(m_chart);
    lay->addLayout(barRow);
    lay->addLayout(btnRow);

    m_outResolution->setText(QStringLiteral("—"));
    m_outFps->setText(QStringLiteral("—"));
    m_outVideo->setText(QStringLiteral("—"));
    m_outAudio->setText(QStringLiteral("—"));
    m_outMux->setText(QStringLiteral("—"));
    m_valPhase->setText(QStringLiteral("—"));
    m_valProgress->setText(QStringLiteral("—"));
    m_valElapsed->setText(QStringLiteral("—"));
    m_valRemaining->setText(QStringLiteral("—"));
    m_valOutput->setText(QStringLiteral("—"));
}

void RenderProgressDialog::setExportInfo(const ProjectSettings &settings)
{
    const QSize r = evenRenderSize(settings.cacheResolution());
    if (m_outResolution) {
        m_outResolution->setText(QStringLiteral("%1 × %2").arg(r.width()).arg(r.height()));
    }
    if (m_outFps) {
        m_outFps->setText(QStringLiteral("%1 fps").arg(settings.fps));
    }
    if (m_outVideo) {
        m_outVideo->setText(QStringLiteral("H.264 (libx264), yuv420p, preset veryfast, CRF 23"));
    }
    if (m_outAudio) {
        m_outAudio->setText(QStringLiteral("AAC, 48 kHz stereo, 192 kbps"));
    }
    if (m_outMux) {
        m_outMux->setText(QStringLiteral("MP4 (+faststart)"));
    }
}

void RenderProgressDialog::appendLog(const QString &line)
{
    auto appendOne = [&line](QPlainTextEdit *ed) {
        if (!ed) {
            return;
        }
        ed->appendPlainText(line);
        QTextCursor c = ed->textCursor();
        c.movePosition(QTextCursor::End);
        ed->setTextCursor(c);
    };
    appendOne(m_log);
    appendOne(m_logFloatPlain);
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

void RenderProgressDialog::onLogWindowClicked()
{
    if (m_logFloatWindow) {
        m_logFloatWindow->raise();
        m_logFloatWindow->activateWindow();
        return;
    }
    auto *dlg = new QDialog(this);
    dlg->setAttribute(Qt::WA_DeleteOnClose);
    dlg->setModal(false);
    dlg->setWindowTitle(tr("Render log"));
    dlg->resize(size());
    dlg->setMinimumSize(minimumSize());
    dlg->setStyleSheet(styleSheet());
    auto *vl = new QVBoxLayout(dlg);
    vl->setContentsMargins(8, 8, 8, 8);
    m_logFloatPlain = new QPlainTextEdit(dlg);
    m_logFloatPlain->setReadOnly(true);
    m_logFloatPlain->setMaximumBlockCount(800);
    if (m_log) {
        m_logFloatPlain->setFont(m_log->font());
        m_logFloatPlain->setPlainText(m_log->toPlainText());
    } else {
        m_logFloatPlain->setFont(monoFont(10));
    }
    vl->addWidget(m_logFloatPlain);
    m_logFloatWindow = dlg;
    connect(dlg, &QObject::destroyed, this, [this]() {
        m_logFloatPlain = nullptr;
    });
    dlg->show();
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
