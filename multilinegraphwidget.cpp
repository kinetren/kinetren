#include "multilinegraphwidget.h"

#include <QPaintEvent>
#include <QPainter>

#include <algorithm>
#include <cmath>
#include <limits>

MultiLineGraphWidget::MultiLineGraphWidget(const QString &title,
                                           const QVector<QString> &seriesNames,
                                           const QVector<QColor> &seriesColors,
                                           const QVector<double> &seriesMax,
                                           MultiLineAxisStyle axisStyle,
                                           int maxSampleHistory,
                                           QWidget *parent)
    : QWidget(parent)
    , m_title(title)
    , m_names(seriesNames)
    , m_colors(seriesColors)
    , m_seriesMax(seriesMax)
    , m_series(static_cast<size_t>(seriesColors.size()))
    , m_axisStyle(axisStyle)
    , m_maxSampleHistory(std::max(8, maxSampleHistory))
{
    m_adaptiveScale.resize(seriesColors.size());
    for (int i = 0; i < m_adaptiveScale.size(); ++i) {
        m_adaptiveScale[i] = std::max(1.0, m_seriesMax.value(i, 100.0) * 0.25);
    }
    setMinimumHeight(128);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
}

void MultiLineGraphWidget::addSample(const QVector<double> &values)
{
    const int count = std::min(values.size(), static_cast<int>(m_colors.size()));
    for (int i = 0; i < count; ++i) {
        m_series[static_cast<size_t>(i)].push_back(values[i]);
        while (m_series[static_cast<size_t>(i)].size() > static_cast<size_t>(m_maxSampleHistory)) {
            m_series[static_cast<size_t>(i)].pop_front();
        }

        if (m_axisStyle != MultiLineAxisStyle::NormalizedPercentGrid) {
            continue;
        }
        const auto &samples = m_series[static_cast<size_t>(i)];
        double recentMax = 0.0;
        for (const double v : samples) {
            recentMax = std::max(recentMax, v);
        }
        const double hardCap = std::max(1.0, m_seriesMax.value(i, 100.0));
        const double floorScale = std::max(1.0, hardCap * 0.02);
        const double targetScale = std::clamp(std::max(floorScale, recentMax * 1.25), floorScale, hardCap);
        const double currentScale = m_adaptiveScale.value(i, floorScale);
        const double alpha = (targetScale > currentScale) ? 0.35 : 0.08;
        m_adaptiveScale[i] = currentScale + alpha * (targetScale - currentScale);
    }
    update();
}

void MultiLineGraphWidget::clearSamples()
{
    for (auto &s : m_series) {
        s.clear();
    }
    update();
}

QVector<MultiLineGraphWidget::PinnedLegend> MultiLineGraphWidget::pinLegends(const QVector<PinnedLegend> &raw,
                                                                           qreal top,
                                                                           qreal bottom,
                                                                           qreal minGap)
{
    if (raw.isEmpty()) {
        return {};
    }

    QVector<PinnedLegend> pinned = raw;
    std::sort(pinned.begin(), pinned.end(), [](const PinnedLegend &a, const PinnedLegend &b) {
        return a.sourceY < b.sourceY;
    });

    pinned[0].pinnedY = std::clamp(pinned[0].sourceY, top, bottom);
    for (int i = 1; i < pinned.size(); ++i) {
        const qreal desired = std::clamp(pinned[i].sourceY, top, bottom);
        pinned[i].pinnedY = std::max(desired, pinned[i - 1].pinnedY + minGap);
    }

    if (pinned.back().pinnedY > bottom) {
        pinned.back().pinnedY = bottom;
        for (int i = pinned.size() - 2; i >= 0; --i) {
            pinned[i].pinnedY = std::min(pinned[i].pinnedY, pinned[i + 1].pinnedY - minGap);
        }
        for (int i = 0; i < pinned.size(); ++i) {
            pinned[i].pinnedY = std::clamp(pinned[i].pinnedY, top, bottom);
        }
    }

    return pinned;
}

void MultiLineGraphWidget::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.fillRect(rect(), QColor(0x1b, 0x1d, 0x20));
    p.setPen(QColor(42, 47, 53));
    p.drawRect(rect().adjusted(0, 0, -1, -1));

    if (m_axisStyle == MultiLineAxisStyle::LinearAutoRange) {
        paintLinearStyle(p, rect());
    } else {
        paintPercentStyle(p, rect());
    }
}

void MultiLineGraphWidget::paintPercentStyle(QPainter &p, const QRect &rect)
{
    const int leftLegendWidth = 112;
    const int axisLabelWidth = 36;
    const int rightValueWidth = 76;
    const int graphLeft = rect.left() + leftLegendWidth + axisLabelWidth + 12;
    const int graphWidth = std::max(80, rect.width() - (leftLegendWidth + axisLabelWidth + rightValueWidth + 20));
    const QRect g(graphLeft, rect.top() + 22, graphWidth, rect.height() - 34);

    p.setPen(QColor(210, 218, 228));
    p.drawText(QRect(rect.left() + 8, rect.top() + 4, rect.width() - 16, 16),
               Qt::AlignLeft | Qt::AlignVCenter,
               m_title);

    p.setPen(QColor(49, 56, 64));
    p.drawLine(g.left(), g.top(), g.right(), g.top());
    p.drawLine(g.left(), g.center().y(), g.right(), g.center().y());
    p.drawLine(g.left(), g.bottom(), g.right(), g.bottom());
    p.setPen(QColor(116, 126, 140));
    p.drawText(QRect(g.left() - axisLabelWidth, g.top() - 7, axisLabelWidth - 4, 16), Qt::AlignRight | Qt::AlignVCenter, "100%");
    p.drawText(QRect(g.left() - axisLabelWidth, g.center().y() - 7, axisLabelWidth - 4, 16), Qt::AlignRight | Qt::AlignVCenter, "50%");
    p.drawText(QRect(g.left() - axisLabelWidth, g.bottom() - 7, axisLabelWidth - 4, 16), Qt::AlignRight | Qt::AlignVCenter, "0%");

    QVector<PinnedLegend> legends;
    legends.reserve(static_cast<int>(m_series.size()));
    for (int si = 0; si < static_cast<int>(m_series.size()); ++si) {
        const auto &samples = m_series[static_cast<size_t>(si)];
        const double maxV = std::max(0.01, m_adaptiveScale.value(si, m_seriesMax.value(si, 100.0)));
        const QColor color = m_colors.value(si, QColor(150, 150, 150));
        const double current = samples.empty() ? 0.0 : samples.back();
        const double currentPct = std::clamp((current / maxV) * 100.0, 0.0, 100.0);

        if (samples.size() < 2) {
            continue;
        }
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setPen(QPen(color, 2));
        for (size_t i = 1; i < samples.size(); ++i) {
            const double x1r = static_cast<double>(i - 1) / static_cast<double>(samples.size() - 1);
            const double x2r = static_cast<double>(i) / static_cast<double>(samples.size() - 1);
            const double y1r = std::clamp((samples[i - 1] / maxV), 0.0, 1.0);
            const double y2r = std::clamp((samples[i] / maxV), 0.0, 1.0);
            const qreal x1 = g.left() + x1r * g.width();
            const qreal x2 = g.left() + x2r * g.width();
            const qreal y1 = g.bottom() - y1r * g.height();
            const qreal y2 = g.bottom() - y2r * g.height();
            p.drawLine(QPointF(x1, y1), QPointF(x2, y2));
        }
        const qreal markerX = g.right();
        const qreal markerY = g.bottom() - (currentPct / 100.0) * g.height();
        p.setBrush(color);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(markerX, markerY), 3.5, 3.5);
        p.setPen(color);
        p.drawText(g.right() + 8, static_cast<int>(markerY) - 2, QString::number(current, 'f', 1));

        const double startPct = std::clamp((samples.front() / maxV) * 100.0, 0.0, 100.0);
        PinnedLegend legend;
        legend.sourceY = g.bottom() - (startPct / 100.0) * g.height();
        legend.pinnedY = legend.sourceY;
        legend.color = color;
        legend.text = m_names.value(si, QString("Series %1").arg(si + 1));
        legends.push_back(legend);
    }

    const QVector<PinnedLegend> pinnedLegends = pinLegends(legends, g.top() + 8, g.bottom() - 8, 14.0);
    const qreal legendLeftX = rect.left() + 8;
    const qreal legendRightX = g.left() - axisLabelWidth - 10;
    const qreal elbowX = g.left() - 6;
    p.setRenderHint(QPainter::Antialiasing, true);
    for (const PinnedLegend &legend : pinnedLegends) {
        p.setPen(QPen(legend.color, 1.4));
        p.drawLine(QPointF(legendRightX, legend.pinnedY), QPointF(elbowX, legend.pinnedY));
        p.drawLine(QPointF(elbowX, legend.pinnedY), QPointF(g.left(), legend.sourceY));

        p.setBrush(legend.color);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(legendLeftX + 4, legend.pinnedY), 3.0, 3.0);

        p.setPen(legend.color);
        p.drawText(QRectF(legendLeftX + 12, legend.pinnedY - 8, std::max(20.0, legendRightX - legendLeftX - 14), 16),
                   Qt::AlignLeft | Qt::AlignVCenter,
                   legend.text);

        p.setBrush(legend.color);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(g.left(), legend.sourceY), 2.2, 2.2);
    }
}

void MultiLineGraphWidget::paintLinearStyle(QPainter &p, const QRect &rect)
{
    const int left = rect.left() + 6;
    const int right = rect.right() - 6;
    const int top = rect.top() + 22;
    const int bottom = rect.bottom() - 8;
    const QRect g(left, top, std::max(1, right - left), std::max(1, bottom - top));

    p.setPen(QColor(210, 218, 228));
    p.drawText(QRect(rect.left() + 8, rect.top() + 4, rect.width() - 16, 16),
               Qt::AlignLeft | Qt::AlignVCenter,
               m_title);

    if (m_series.empty() || m_series[0].empty()) {
        p.setPen(QColor(120, 128, 140));
        p.drawText(g, Qt::AlignCenter, QStringLiteral("—"));
        return;
    }

    double vmin = std::numeric_limits<double>::infinity();
    double vmax = -std::numeric_limits<double>::infinity();
    for (const auto &s : m_series) {
        for (double v : s) {
            if (!std::isfinite(v)) {
                continue;
            }
            vmin = std::min(vmin, v);
            vmax = std::max(vmax, v);
        }
    }
    if (!std::isfinite(vmin) || !std::isfinite(vmax)) {
        p.setPen(QColor(120, 128, 140));
        p.drawText(g, Qt::AlignCenter, QStringLiteral("—"));
        return;
    }
    const double pad = std::max(0.05 * (vmax - vmin), 0.02 * std::max(vmax, 0.1));
    double y0 = std::max(0.0, vmin - pad);
    double y1 = vmax + pad;
    if (y1 <= y0) {
        y1 = y0 + 0.01;
    }

    p.setPen(QPen(QColor(45, 52, 62), 1, Qt::DotLine));
    for (int i = 1; i < 4; ++i) {
        const int y = g.top() + (g.height() * i) / 4;
        p.drawLine(g.left(), y, g.right(), y);
    }

    p.setRenderHint(QPainter::Antialiasing, true);
    for (int si = 0; si < static_cast<int>(m_series.size()); ++si) {
        const auto &samples = m_series[static_cast<size_t>(si)];
        if (samples.size() < 2) {
            continue;
        }
        const QColor color = m_colors.value(si, QColor(88, 198, 255));
        p.setPen(QPen(color, 2));
        const size_t n = samples.size();
        for (size_t i = 1; i < n; ++i) {
            const double t1 = (n == 1) ? 0.0 : static_cast<double>(i - 1) / static_cast<double>(n - 1);
            const double t2 = (n == 1) ? 1.0 : static_cast<double>(i) / static_cast<double>(n - 1);
            const double v1 = samples[i - 1];
            const double v2 = samples[i];
            const double ny1 = (v1 - y0) / (y1 - y0);
            const double ny2 = (v2 - y0) / (y1 - y0);
            const qreal x1 = g.left() + t1 * g.width();
            const qreal x2 = g.left() + t2 * g.width();
            const qreal yA = g.bottom() - std::clamp(ny1, 0.0, 1.0) * g.height();
            const qreal yB = g.bottom() - std::clamp(ny2, 0.0, 1.0) * g.height();
            p.drawLine(QPointF(x1, yA), QPointF(x2, yB));
        }
        if (!samples.empty()) {
            const double cur = samples.back();
            const double ny = (cur - y0) / (y1 - y0);
            const qreal mx = g.right();
            const qreal my = g.bottom() - std::clamp(ny, 0.0, 1.0) * g.height();
            p.setBrush(color);
            p.setPen(Qt::NoPen);
            p.drawEllipse(QPointF(mx, my), 3.5, 3.5);
            p.setPen(color);
            p.drawText(g.right() + 6, static_cast<int>(my) - 2, QString::number(cur, 'f', 2));
        }
    }

    p.setPen(QColor(160, 170, 185));
    const QFont f = p.font();
    p.setFont(QFont(f.family(), 8));
    p.drawText(g.adjusted(8, 4, -8, -4), Qt::AlignTop | Qt::AlignLeft, QStringLiteral("%1").arg(y1, 0, 'f', 2));
    p.drawText(g.adjusted(8, 4, -8, -4), Qt::AlignBottom | Qt::AlignLeft, QStringLiteral("%1").arg(y0, 0, 'f', 2));

    int ly = g.top() + 4;
    for (int si = 0; si < m_names.size() && si < m_colors.size(); ++si) {
        p.setPen(m_colors[si]);
        p.drawText(QRect(g.left() + 4, ly, g.width() - 8, 14), Qt::AlignLeft | Qt::AlignTop, m_names[si]);
        ly += 14;
    }
}
