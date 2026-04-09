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
    explicit MultiLineGraphWidget(const QString &title,
                                  const QVector<QString> &seriesNames,
                                  const QVector<QColor> &seriesColors,
                                  const QVector<double> &seriesMax,
                                  QWidget *parent = nullptr)
        : QWidget(parent)
        , title(title)
        , names(seriesNames)
        , colors(seriesColors)
        , seriesMax(seriesMax)
        , series(static_cast<size_t>(seriesColors.size()))
    {
        setMinimumHeight(150);
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
        const int legendWidth = std::min(320, std::max(220, width() / 3));
        const QRect legendRect(rect().left() + 8, rect().top() + 8, legendWidth - 16, rect().height() - 16);
        const QRect g(legendRect.right() + 12, rect().top() + 20, rect().right() - 8 - (legendRect.right() + 12), rect().height() - 32);

        p.setPen(QColor(210, 218, 228));
        p.drawText(legendRect.adjusted(0, 0, 0, -legendRect.height() + 16), Qt::AlignLeft | Qt::AlignTop, title);
        p.setPen(QColor(140, 150, 164));
        p.drawText(legendRect.adjusted(0, 14, 0, -legendRect.height() + 30),
                   Qt::AlignLeft | Qt::AlignTop,
                   "Normalized (0-100%)");

        p.setPen(QColor(49, 56, 64));
        p.drawLine(g.left(), g.top(), g.right(), g.top());
        p.drawLine(g.left(), g.center().y(), g.right(), g.center().y());
        p.drawLine(g.left(), g.bottom(), g.right(), g.bottom());
        p.setPen(QColor(116, 126, 140));
        p.drawText(g.left() - 34, g.top() + 4, "100%");
        p.drawText(g.left() - 30, g.center().y() + 4, "50%");
        p.drawText(g.left() - 24, g.bottom() + 4, "0%");

        for (int si = 0; si < static_cast<int>(series.size()); ++si) {
            const auto &samples = series[static_cast<size_t>(si)];
            const double maxV = std::max(0.01, seriesMax.value(si, 100.0));
            const QColor color = colors.value(si, QColor(150, 150, 150));
            const QString name = names.value(si, QString("S%1").arg(si + 1));
            const double current = samples.empty() ? 0.0 : samples.back();
            const double currentPct = std::clamp((current / maxV) * 100.0, 0.0, 100.0);

            const int rowY = legendRect.top() + 36 + si * 20;
            p.setPen(Qt::NoPen);
            p.setBrush(color);
            p.drawRect(legendRect.left(), rowY + 4, 10, 10);
            p.setPen(QColor(210, 218, 228));
            p.drawText(legendRect.left() + 16,
                       rowY + 13,
                       QString("%1: %2 / %3 (%4%)")
                           .arg(name)
                           .arg(current, 0, 'f', 1)
                           .arg(maxV, 0, 'f', 0)
                           .arg(currentPct, 0, 'f', 0));

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
        }
    }

private:
    static constexpr size_t kMaxSamples = 180;
    QString title;
    QVector<QString> names;
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
    qualityGraph = new MultiLineGraphWidget(
        "Quality Metrics",
        {"FPS", "CacheHit", "FinalHit", "AudioHit"},
        {QColor(78, 200, 110), QColor(74, 166, 255), QColor(250, 190, 76), QColor(181, 132, 255)},
        {120.0, 100.0, 100.0, 100.0},
        this);
    activityGraph = new MultiLineGraphWidget(
        "avcpp Activity",
        {"Total/s", "Open/s", "Read/s", "VDec/s", "ADec/s"},
        {QColor(255, 145, 80), QColor(255, 205, 105), QColor(244, 99, 99), QColor(95, 209, 155), QColor(130, 173, 255)},
        {5000.0, 500.0, 5000.0, 1500.0, 1500.0},
        this);

    layout->addWidget(title);
    layout->addWidget(composeFpsLabel);
    layout->addWidget(cacheStatsLabel);
    layout->addWidget(cacheResolutionLabel);
    layout->addWidget(projectInfoLabel);
    layout->addWidget(qualityGraph);
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
