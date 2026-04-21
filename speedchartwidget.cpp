#include "speedchartwidget.h"

#include <QPainter>
#include <QPaintEvent>
#include <QPolygonF>

#include <algorithm>
#include <cmath>

SpeedChartWidget::SpeedChartWidget(QWidget *parent)
    : QWidget(parent)
{
    setMinimumHeight(128);
    setMaximumHeight(200);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
}

QSize SpeedChartWidget::sizeHint() const
{
    return QSize(600, 168);
}

QSize SpeedChartWidget::minimumSizeHint() const
{
    return QSize(320, 128);
}

void SpeedChartWidget::clear()
{
    m_samples.clear();
    update();
}

void SpeedChartWidget::addSample(double speedRealtimeFactor)
{
    if (!std::isfinite(speedRealtimeFactor) || speedRealtimeFactor <= 0.0) {
        return;
    }
    m_samples.push_back(speedRealtimeFactor);
    while (m_samples.size() > kMaxSamples) {
        m_samples.removeFirst();
    }
    update();
}

void SpeedChartWidget::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QRect r = rect().adjusted(1, 1, -1, -1);
    p.fillRect(r, QColor(22, 24, 28));
    p.setPen(QPen(QColor(55, 62, 72), 1));
    p.drawRect(r);

    if (m_samples.isEmpty()) {
        p.setPen(QColor(120, 128, 140));
        p.drawText(r, Qt::AlignCenter, QStringLiteral("—"));
        return;
    }

    double vmin = m_samples.first();
    double vmax = m_samples.first();
    for (double v : m_samples) {
        vmin = std::min(vmin, v);
        vmax = std::max(vmax, v);
    }
    const double pad = std::max(0.05 * (vmax - vmin), 0.02 * std::max(vmax, 0.1));
    double y0 = std::max(0.0, vmin - pad);
    double y1 = vmax + pad;
    if (y1 <= y0) {
        y1 = y0 + 0.01;
    }

    const int n = m_samples.size();
    const int left = r.left() + 4;
    const int right = r.right() - 4;
    const int top = r.top() + 6;
    const int bottom = r.bottom() - 6;
    const int wPlot = std::max(1, right - left);
    const int hPlot = std::max(1, bottom - top);

    p.setPen(QPen(QColor(45, 52, 62), 1, Qt::DotLine));
    for (int i = 1; i < 4; ++i) {
        const int y = top + (hPlot * i) / 4;
        p.drawLine(left, y, right, y);
    }

    QPolygonF poly;
    poly.reserve(n);
    for (int i = 0; i < n; ++i) {
        const double t = (n == 1) ? 0.5 : static_cast<double>(i) / static_cast<double>(n - 1);
        const double v = m_samples[static_cast<int>(i)];
        const double ny = (v - y0) / (y1 - y0);
        const double x = left + t * wPlot;
        const double y = bottom - ny * hPlot;
        poly.append(QPointF(x, y));
    }

    p.setPen(QPen(QColor(88, 198, 255), 2));
    p.drawPolyline(poly);

    p.setPen(QColor(160, 170, 185));
    const QFont f = p.font();
    p.setFont(QFont(f.family(), 8));
    p.drawText(r.adjusted(8, 4, -8, -4), Qt::AlignTop | Qt::AlignLeft,
               QStringLiteral("%1×").arg(y1, 0, 'f', 2));
    p.drawText(r.adjusted(8, 4, -8, -4), Qt::AlignBottom | Qt::AlignLeft,
               QStringLiteral("%1×").arg(y0, 0, 'f', 2));
}
