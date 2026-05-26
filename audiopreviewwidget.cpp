#include "audiopreviewwidget.h"

#include <QPainter>

#include <algorithm>
#include <cmath>

AudioPreviewWidget::AudioPreviewWidget(QWidget *parent)
    : QWidget(parent)
{
    setMinimumWidth(88);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
}

void AudioPreviewWidget::setPeaks(const QVector<QPair<int, int>> &stereoColumns)
{
    peaks = stereoColumns;
    statusText.clear();
    update();
}

void AudioPreviewWidget::setPlayheadRel(double rel01)
{
    playheadRel = rel01;
    update();
}

void AudioPreviewWidget::setStatusText(const QString &text)
{
    statusText = text;
    peaks.clear();
    update();
}

void AudioPreviewWidget::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.fillRect(rect(), QColor(16, 18, 20));

    const QRect inner = rect().adjusted(4, 4, -4, -4);
    if (!statusText.isEmpty()) {
        p.setPen(QColor(140, 150, 165));
        p.drawText(inner, Qt::AlignCenter | Qt::TextWordWrap, statusText);
        return;
    }
    if (peaks.isEmpty()) {
        p.setPen(QColor(100, 110, 125));
        p.drawText(inner, Qt::AlignCenter, tr("Master mix…"));
        return;
    }

    const int midY = inner.center().y();
    const int halfH = std::max(2, inner.height() / 2 - 2);
    const int cols = peaks.size();
    for (int i = 0; i < cols; ++i) {
        const int x = inner.left() +
                      (cols <= 1 ? 0 : (i * std::max(1, inner.width() - 1)) / (cols - 1));
        const int ampL = std::clamp(peaks[i].first, 1, halfH);
        const int ampR = std::clamp(peaks[i].second, 1, halfH);
        p.setPen(QColor(170, 210, 255, 230));
        p.drawLine(x, midY - ampL, x, midY - 1);
        p.setPen(QColor(230, 190, 255, 230));
        p.drawLine(x, midY + 1, x, midY + ampR);
    }

    if (playheadRel >= 0.0) {
        const double t = std::clamp(playheadRel, 0.0, 1.0);
        const int px = inner.left() + static_cast<int>(std::llround(t * std::max(0, inner.width() - 1)));
        p.setPen(QPen(QColor(255, 220, 120), 2));
        p.drawLine(px, inner.top(), px, inner.bottom());
    }

    p.setPen(QColor(52, 58, 66));
    p.drawRect(rect().adjusted(0, 0, -1, -1));
}
