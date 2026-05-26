#include "previewaudiopanel.h"

#include <QFontMetrics>
#include <QLabel>
#include <QResizeEvent>
#include <QStackedLayout>

#include <algorithm>

PreviewAudioPanel::PreviewAudioPanel(QWidget *parent)
    : QWidget(parent)
{
    setObjectName(QStringLiteral("PreviewAudioPanel"));

    auto *stack = new QStackedLayout(this);
    stack->setStackingMode(QStackedLayout::StackAll);
    stack->setContentsMargins(0, 0, 0, 0);

    metersRow = new QWidget(this);
    metersRow->setObjectName(QStringLiteral("PreviewAudioMetersRow"));
    stack->addWidget(metersRow);

    volumeOverlay = new QLabel(this);
    volumeOverlay->setObjectName(QStringLiteral("PreviewVolumeDbOverlay"));
    volumeOverlay->setAttribute(Qt::WA_TransparentForMouseEvents);
    volumeOverlay->setAlignment(Qt::AlignHCenter | Qt::AlignTop);
    volumeOverlay->setText(QStringLiteral("0 dB"));
    volumeOverlay->setStyleSheet(
        QStringLiteral("QLabel#PreviewVolumeDbOverlay {"
                       "  color: #f4f7fc;"
                       "  font-size: 26px;"
                       "  font-weight: 700;"
                       "  background: transparent;"
                       "}"));
    stack->addWidget(volumeOverlay);
}

void PreviewAudioPanel::setVolumeOverlayText(const QString &text)
{
    if (volumeOverlay) {
        volumeOverlay->setText(text);
        repositionOverlay();
    }
}

void PreviewAudioPanel::resizeEvent(QResizeEvent *event)
{
    QWidget::resizeEvent(event);
    repositionOverlay();
}

void PreviewAudioPanel::repositionOverlay()
{
    if (!volumeOverlay) {
        return;
    }
    const QFontMetrics fm(volumeOverlay->fontMetrics());
    const int tw = fm.horizontalAdvance(volumeOverlay->text()) + 4;
    const int th = fm.height() + 4;
    const int x = std::max(0, (width() - tw) / 2);
    volumeOverlay->setGeometry(x, 6, tw, th);
    volumeOverlay->raise();
}
