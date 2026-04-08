#include "performancewidget.h"

#include <QLabel>
#include <QPainter>
#include <QVBoxLayout>
#include <algorithm>
#include <deque>

class CombinedMetricGraphWidget : public QWidget
{
public:
    explicit CombinedMetricGraphWidget(QWidget *parent = nullptr)
        : QWidget(parent)
    {
        setMinimumHeight(128);
    }

    void setSample(double fps, double cacheHitPct, double avcppCallsPerSecond)
    {
        fpsSamples.push_back(fps);
        cacheSamples.push_back(cacheHitPct);
        avcppSamples.push_back(avcppCallsPerSecond);
        while (fpsSamples.size() > kMaxSamples) {
            fpsSamples.pop_front();
            cacheSamples.pop_front();
            avcppSamples.pop_front();
        }
        update();
    }

protected:
    void paintEvent(QPaintEvent *event) override
    {
        Q_UNUSED(event);
        QPainter p(this);
        p.fillRect(rect(), QColor(24, 27, 31));
        p.setPen(QColor(61, 68, 77));
        p.drawRect(rect().adjusted(0, 0, -1, -1));

        const QRect graphRect = rect().adjusted(8, 20, -8, -10);
        p.setPen(QColor(49, 56, 64));
        p.drawLine(graphRect.left(), graphRect.center().y(), graphRect.right(), graphRect.center().y());
        p.drawLine(graphRect.left(), graphRect.top(), graphRect.right(), graphRect.top());
        p.drawLine(graphRect.left(), graphRect.bottom(), graphRect.right(), graphRect.bottom());

        if (fpsSamples.empty()) {
            p.setPen(QColor(196, 205, 216));
            p.drawText(rect().adjusted(8, 1, -8, -1), Qt::AlignLeft | Qt::AlignTop, "Merged Metrics: -");
            return;
        }

        const double fpsNow = fpsSamples.back();
        const double cacheNow = cacheSamples.back();
        const double avcppNow = avcppSamples.back();
        p.setPen(colorFor(fpsNow / 120.0, false));
        p.drawText(rect().adjusted(8, 1, -8, -1), Qt::AlignLeft | Qt::AlignTop, QString("FPS %1").arg(fpsNow, 0, 'f', 1));
        p.setPen(colorFor(cacheNow / 100.0, false));
        p.drawText(rect().adjusted(100, 1, -8, -1), Qt::AlignLeft | Qt::AlignTop, QString("Cache %1%").arg(cacheNow, 0, 'f', 1));
        p.setPen(colorFor(avcppNow / 5000.0, true));
        p.drawText(rect().adjusted(220, 1, -8, -1), Qt::AlignLeft | Qt::AlignTop, QString("avcpp/s %1").arg(avcppNow, 0, 'f', 1));

        if (fpsSamples.size() < 2) {
            return;
        }

        const auto drawSeries = [&](const std::deque<double> &series, double minV, double maxV, const QColor &lineColor) {
            p.setRenderHint(QPainter::Antialiasing, true);
            p.setPen(QPen(lineColor, 2));
            for (size_t i = 1; i < series.size(); ++i) {
                const double x1Ratio = static_cast<double>(i - 1) / static_cast<double>(series.size() - 1);
                const double x2Ratio = static_cast<double>(i) / static_cast<double>(series.size() - 1);
                const double y1Norm = std::clamp((series[i - 1] - minV) / std::max(0.01, maxV - minV), 0.0, 1.0);
                const double y2Norm = std::clamp((series[i] - minV) / std::max(0.01, maxV - minV), 0.0, 1.0);
                const qreal x1 = graphRect.left() + x1Ratio * graphRect.width();
                const qreal x2 = graphRect.left() + x2Ratio * graphRect.width();
                const qreal y1 = graphRect.bottom() - y1Norm * graphRect.height();
                const qreal y2 = graphRect.bottom() - y2Norm * graphRect.height();
                p.drawLine(QPointF(x1, y1), QPointF(x2, y2));
            }
        };

        drawSeries(fpsSamples, 0.0, 120.0, QColor(78, 200, 110));
        drawSeries(cacheSamples, 0.0, 100.0, QColor(74, 166, 255));
        drawSeries(avcppSamples, 0.0, 5000.0, QColor(233, 154, 72));
    }

private:
    static QColor colorFor(double normalizedValue, bool lowerIsBetter)
    {
        const double score = lowerIsBetter ? (1.0 - normalizedValue) : normalizedValue;
        if (score < 0.35) {
            return QColor(225, 72, 72);
        }
        if (score < 0.65) {
            return QColor(225, 171, 56);
        }
        return QColor(75, 180, 92);
    }

    static constexpr size_t kMaxSamples = 120;
    std::deque<double> fpsSamples;
    std::deque<double> cacheSamples;
    std::deque<double> avcppSamples;
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
    combinedGraph = new CombinedMetricGraphWidget(this);

    layout->addWidget(title);
    layout->addWidget(composeFpsLabel);
    layout->addWidget(cacheStatsLabel);
    layout->addWidget(cacheResolutionLabel);
    layout->addWidget(projectInfoLabel);
    layout->addWidget(combinedGraph);
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
    combinedGraph->setSample(fpsValue, cacheHitPct, avcppCallsPerSecond);
}
