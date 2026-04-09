#include "performancewidget.h"

#include <QLabel>
#include <QPainter>
#include <QVBoxLayout>
#include <algorithm>
#include <deque>
#include <vector>

class MultiLineGraphWidget : public QWidget
{
public:
    explicit MultiLineGraphWidget(const QVector<QColor> &colors,
                                  const QVector<double> &seriesMax,
                                  QWidget *parent = nullptr)
        : QWidget(parent)
        , colors(colors)
        , seriesMax(seriesMax)
        , series(static_cast<size_t>(colors.size()))
    {
        setMinimumHeight(110);
    }

    void addSample(const QVector<double> &values)
    {
        const int count = std::min(values.size(), colors.size());
        for (int i = 0; i < count; ++i) {
            series[static_cast<size_t>(i)].push_back(values[i]);
            while (series[static_cast<size_t>(i)].size() > kMaxSamples) {
                series[static_cast<size_t>(i)].pop_front();
            }
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
        const QRect g = rect().adjusted(8, 8, -8, -8);
        p.setPen(QColor(49, 56, 64));
        p.drawLine(g.left(), g.top(), g.right(), g.top());
        p.drawLine(g.left(), g.center().y(), g.right(), g.center().y());
        p.drawLine(g.left(), g.bottom(), g.right(), g.bottom());

        for (int si = 0; si < static_cast<int>(series.size()); ++si) {
            const auto &samples = series[static_cast<size_t>(si)];
            if (samples.size() < 2) {
                continue;
            }
            const double maxV = std::max(0.01, seriesMax.value(si, 100.0));
            p.setRenderHint(QPainter::Antialiasing, true);
            p.setPen(QPen(colors.value(si, QColor(150, 150, 150)), 2));
            for (size_t i = 1; i < samples.size(); ++i) {
                const double x1r = static_cast<double>(i - 1) / static_cast<double>(samples.size() - 1);
                const double x2r = static_cast<double>(i) / static_cast<double>(samples.size() - 1);
                const double y1r = std::clamp(samples[i - 1] / maxV, 0.0, 1.0);
                const double y2r = std::clamp(samples[i] / maxV, 0.0, 1.0);
                const qreal x1 = g.left() + x1r * g.width();
                const qreal x2 = g.left() + x2r * g.width();
                const qreal y1 = g.bottom() - y1r * g.height();
                const qreal y2 = g.bottom() - y2r * g.height();
                p.drawLine(QPointF(x1, y1), QPointF(x2, y2));
            }
        }
    }

private:
    static constexpr size_t kMaxSamples = 180;
    QVector<QColor> colors;
    QVector<double> seriesMax;
    std::vector<std::deque<double>> series;
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
    qualityLegendLabel = new QLabel("Quality graph: FPS | cache hit | final hit | audio hit", this);
    qualityLegendLabel->setStyleSheet("color:#d0d8e2;");
    activityLegendLabel = new QLabel("avcpp graph: total/s | open/s | read/s | vdec/s | adec/s", this);
    activityLegendLabel->setStyleSheet("color:#d0d8e2;");
    qualityGraph = new MultiLineGraphWidget(
        {QColor(78, 200, 110), QColor(74, 166, 255), QColor(250, 190, 76), QColor(181, 132, 255)},
        {120.0, 100.0, 100.0, 100.0},
        this);
    activityGraph = new MultiLineGraphWidget(
        {QColor(255, 145, 80), QColor(255, 205, 105), QColor(244, 99, 99), QColor(95, 209, 155), QColor(130, 173, 255)},
        {5000.0, 500.0, 5000.0, 1500.0, 1500.0},
        this);

    layout->addWidget(title);
    layout->addWidget(composeFpsLabel);
    layout->addWidget(cacheStatsLabel);
    layout->addWidget(cacheResolutionLabel);
    layout->addWidget(projectInfoLabel);
    layout->addWidget(qualityLegendLabel);
    layout->addWidget(qualityGraph);
    layout->addWidget(activityLegendLabel);
    layout->addWidget(activityGraph);
}

void PerformanceWidget::setMetrics(const QString &composeFps,
                                   const QString &cacheStats,
                                   const QString &cacheResolution,
                                   const QString &projectInfo,
                                   double fpsValue,
                                   double cacheHitPct,
                                   double finalCacheHitPct,
                                   double audioCacheHitPct,
                                   double avcppCallsPerSecond,
                                   double avcppOpenPerSec,
                                   double avcppReadPerSec,
                                   double avcppVideoDecodePerSec,
                                   double avcppAudioDecodePerSec)
{
    composeFpsLabel->setText(composeFps);
    cacheStatsLabel->setText(cacheStats);
    cacheResolutionLabel->setText(cacheResolution);
    projectInfoLabel->setText(projectInfo);
    qualityGraph->addSample({fpsValue, cacheHitPct, finalCacheHitPct, audioCacheHitPct});
    activityGraph->addSample({avcppCallsPerSecond,
                              avcppOpenPerSec,
                              avcppReadPerSec,
                              avcppVideoDecodePerSec,
                              avcppAudioDecodePerSec});
}
