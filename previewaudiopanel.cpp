#include "previewaudiopanel.h"

#include <QFont>
#include <QFontMetrics>
#include <QLabel>
#include <QResizeEvent>
#include <QVBoxLayout>

PreviewAudioPanel::PreviewAudioPanel(QWidget *parent)
    : QWidget(parent)
{
    setObjectName(QStringLiteral("PreviewAudioPanel"));

    metersRow = new QWidget(this);
    metersRow->setObjectName(QStringLiteral("PreviewAudioMetersRow"));
    auto *panelLayout = new QVBoxLayout(this);
    panelLayout->setContentsMargins(0, 0, 0, 0);
    panelLayout->setSpacing(0);
    panelLayout->addWidget(metersRow);

    volumeOverlay = new QLabel(this);
    volumeOverlay->setObjectName(QStringLiteral("PreviewVolumeDbOverlay"));
    volumeOverlay->setAttribute(Qt::WA_TransparentForMouseEvents);
    volumeOverlay->setAlignment(Qt::AlignHCenter | Qt::AlignTop);
    volumeOverlay->setText(QStringLiteral("0 dB"));
    QFont overlayFont = volumeOverlay->font();
    overlayFont.setPixelSize(26);
    overlayFont.setWeight(QFont::Bold);
    volumeOverlay->setFont(overlayFont);
    volumeOverlay->setStyleSheet(
        QStringLiteral("QLabel#PreviewVolumeDbOverlay {"
                       "  color: #f4f7fc;"
                       "  background: transparent;"
                       "}"));
    const QFontMetrics fm(overlayFont);
    overlayBandHeight_ = fm.height() + 4;
    repositionOverlay();
}

void PreviewAudioPanel::setVolumeOverlayText(const QString &text)
{
    if (volumeOverlay) {
        volumeOverlay->setText(text);
        volumeOverlay->raise();
    }
}

void PreviewAudioPanel::resizeEvent(QResizeEvent *event)
{
    QWidget::resizeEvent(event);
    repositionOverlay();
}

void PreviewAudioPanel::repositionOverlay()
{
    if (!volumeOverlay || overlayBandHeight_ <= 0) {
        return;
    }
    volumeOverlay->setGeometry(0, 6, width(), overlayBandHeight_);
    volumeOverlay->raise();
}
