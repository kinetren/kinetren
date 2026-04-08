#include "performancewidget.h"

#include <QLabel>
#include <QPainter>
#include <QPainterPath>
#include <QVBoxLayout>
#include <algorithm>
#include <deque>

class MetricGraphWidget : public QWidget
{
public:
    explicit MetricGraphWidget(const QString &title, bool lowerIsBetter, QWidget *parent = nullptr)
        : QWidget(parent)
        , title(title)
        , lowerIsBetter(lowerIsBetter)
    {
        setMinimumHeight(44);
    }

    void setSample(double value)
    {
        samples.push_back(value);
        while (samples.size() > kMaxSamples) {
            samples.pop_front();
        }
        update();
    }

    void setRange(double minValue, double maxValue)
    {
        rangeMin = minValue;
        rangeMax = std::max(minValue + 0.01, maxValue);
    }

protected:
    void paintEvent(QPaintEvent *event) override
    {
        Q_UNUSED(event);
        QPainter p(this);
        p.fillRect(rect(), QColor(24, 27, 31));
        p.setPen(QColor(61, 68, 77));
        p.drawRect(rect().adjusted(0, 0, -1, -1));

        const QRect graphRect = rect().adjusted(6, 16, -6, -6);
        p.setPen(QColor(49, 56, 64));
        p.drawLine(graphRect.left(), graphRect.center().y(), graphRect.right(), graphRect.center().y());

        if (samples.empty()) {
            p.setPen(QColor(196, 205, 216));
            p.drawText(rect().adjusted(6, 1, -6, -1), Qt::AlignLeft | Qt::AlignTop, title + ": -");
            return;
        }

        const double current = samples.back();
        QColor accent = QColor(75, 180, 92);
        if (scoreFor(current) < 0.35) {
            accent = QColor(225, 72, 72);
        } else if (scoreFor(current) < 0.65) {
            accent = QColor(225, 171, 56);
        }

        p.setPen(accent);
        p.drawText(rect().adjusted(6, 1, -6, -1),
                   Qt::AlignLeft | Qt::AlignTop,
                   QString("%1: %2").arg(title).arg(current, 0, 'f', 1));

        if (samples.size() < 2) {
            return;
        }

        QPainterPath path;
        for (size_t i = 0; i < samples.size(); ++i) {
            const double xRatio = static_cast<double>(i) / static_cast<double>(samples.size() - 1);
            const double yRatio = normalized(samples[i]);
            const qreal x = graphRect.left() + xRatio * graphRect.width();
            const qreal y = graphRect.bottom() - yRatio * graphRect.height();
            if (i == 0) {
                path.moveTo(x, y);
            } else {
                path.lineTo(x, y);
            }
        }
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setPen(QPen(accent, 2));
        p.drawPath(path);
    }

private:
    double normalized(double value) const
    {
        const double n = (value - rangeMin) / (rangeMax - rangeMin);
        return std::clamp(n, 0.0, 1.0);
    }

    double scoreFor(double value) const
    {
        const double n = normalized(value);
        return lowerIsBetter ? (1.0 - n) : n;
    }

    static constexpr size_t kMaxSamples = 90;
    QString title;
    bool lowerIsBetter = false;
    double rangeMin = 0.0;
    double rangeMax = 100.0;
    std::deque<double> samples;
};

PerformanceWidget::PerformanceWidget(QWidget *parent)
    : QWidget(parent)
{
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(8, 8, 8, 8);
    layout->setSpacing(3);

    auto *title = new QLabel("Performance / Utility", this);
    title->setStyleSheet("font-weight:600; color:#d9e3ef;");
    composeFpsLabel = new QLabel("Compose FPS: -", this);
    cacheStatsLabel = new QLabel("Cache: -", this);
    cacheResolutionLabel = new QLabel("Cache Resolution: -", this);
    projectInfoLabel = new QLabel("Project: -", this);
    fpsGraph = new MetricGraphWidget("Compose FPS", false, this);
    cacheHitGraph = new MetricGraphWidget("Cache Hit %", false, this);
    avcppGraph = new MetricGraphWidget("avcpp calls/s", true, this);
    fpsGraph->setRange(0.0, 120.0);
    cacheHitGraph->setRange(0.0, 100.0);
    avcppGraph->setRange(0.0, 5000.0);

    layout->addWidget(title);
    layout->addWidget(composeFpsLabel);
    layout->addWidget(cacheStatsLabel);
    layout->addWidget(cacheResolutionLabel);
    layout->addWidget(projectInfoLabel);
    layout->addWidget(fpsGraph);
    layout->addWidget(cacheHitGraph);
    layout->addWidget(avcppGraph);
}

void PerformanceWidget::setMetrics(const QString &composeFps,
                                   const QString &cacheStats,
                                   const QString &cacheResolution,
                                   const QString &projectInfo,
                                   double fpsValue,
                                   double cacheHitPct,
                                   double avcppCallsPerSecond)
{
    composeFpsLabel->setText(composeFps);
    cacheStatsLabel->setText(cacheStats);
    cacheResolutionLabel->setText(cacheResolution);
    projectInfoLabel->setText(projectInfo);
    fpsGraph->setSample(fpsValue);
    cacheHitGraph->setSample(cacheHitPct);
    avcppGraph->setSample(avcppCallsPerSecond);
}
