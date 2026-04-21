#include "performancewidget.h"

#include <QGuiApplication>
#include <QLabel>
#include <QPainter>
#include <QScreen>
#include <QSizePolicy>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <algorithm>
#include <deque>
#include <vector>
#include <cmath>

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
        adaptiveScale.resize(seriesColors.size());
        for (int i = 0; i < adaptiveScale.size(); ++i) {
            adaptiveScale[i] = std::max(1.0, seriesMax.value(i, 100.0) * 0.25);
        }
        setMinimumHeight(170);
        setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    }

    void addSample(const QVector<double> &values)
    {
        const int count = std::min(values.size(), colors.size());
        for (int i = 0; i < count; ++i) {
            series[static_cast<size_t>(i)].push_back(values[i]);
            while (series[static_cast<size_t>(i)].size() > kMaxSamples) {
                series[static_cast<size_t>(i)].pop_front();
            }

            const auto &samples = series[static_cast<size_t>(i)];
            double recentMax = 0.0;
            for (const double v : samples) {
                recentMax = std::max(recentMax, v);
            }
            const double hardCap = std::max(1.0, seriesMax.value(i, 100.0));
            const double floorScale = std::max(1.0, hardCap * 0.02);
            const double targetScale = std::clamp(std::max(floorScale, recentMax * 1.25), floorScale, hardCap);
            const double currentScale = adaptiveScale.value(i, floorScale);
            const double alpha = (targetScale > currentScale) ? 0.35 : 0.08;
            adaptiveScale[i] = currentScale + alpha * (targetScale - currentScale);
        }
        update();
    }

protected:
    struct PinnedLegend {
        qreal sourceY = 0.0;
        qreal pinnedY = 0.0;
        QString text;
        QColor color;
    };

    static QVector<PinnedLegend> pinLegends(const QVector<PinnedLegend> &raw,
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

    void paintEvent(QPaintEvent *event) override
    {
        Q_UNUSED(event);
        QPainter p(this);
        p.fillRect(rect(), QColor(0x1b, 0x1d, 0x20));
        p.setPen(QColor(42, 47, 53));
        p.drawRect(rect().adjusted(0, 0, -1, -1));
        const int leftLegendWidth = 112;
        const int axisLabelWidth = 36;
        const int rightValueWidth = 76;
        const int graphLeft = rect().left() + leftLegendWidth + axisLabelWidth + 12;
        const int graphWidth = std::max(80, rect().width() - (leftLegendWidth + axisLabelWidth + rightValueWidth + 20));
        const QRect g(graphLeft, rect().top() + 22, graphWidth, rect().height() - 34);

        p.setPen(QColor(210, 218, 228));
        p.drawText(QRect(rect().left() + 8, rect().top() + 4, rect().width() - 16, 16),
                   Qt::AlignLeft | Qt::AlignVCenter,
                   title);

        p.setPen(QColor(49, 56, 64));
        p.drawLine(g.left(), g.top(), g.right(), g.top());
        p.drawLine(g.left(), g.center().y(), g.right(), g.center().y());
        p.drawLine(g.left(), g.bottom(), g.right(), g.bottom());
        p.setPen(QColor(116, 126, 140));
        p.drawText(QRect(g.left() - axisLabelWidth, g.top() - 7, axisLabelWidth - 4, 16), Qt::AlignRight | Qt::AlignVCenter, "100%");
        p.drawText(QRect(g.left() - axisLabelWidth, g.center().y() - 7, axisLabelWidth - 4, 16), Qt::AlignRight | Qt::AlignVCenter, "50%");
        p.drawText(QRect(g.left() - axisLabelWidth, g.bottom() - 7, axisLabelWidth - 4, 16), Qt::AlignRight | Qt::AlignVCenter, "0%");

        QVector<PinnedLegend> legends;
        legends.reserve(static_cast<int>(series.size()));
        for (int si = 0; si < static_cast<int>(series.size()); ++si) {
            const auto &samples = series[static_cast<size_t>(si)];
            const double maxV = std::max(0.01, adaptiveScale.value(si, seriesMax.value(si, 100.0)));
            const QColor color = colors.value(si, QColor(150, 150, 150));
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
            legend.text = names.value(si, QString("Series %1").arg(si + 1));
            legends.push_back(legend);
        }

        const QVector<PinnedLegend> pinnedLegends = pinLegends(legends, g.top() + 8, g.bottom() - 8, 14.0);
        const qreal legendLeftX = rect().left() + 8;
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

private:
    static constexpr size_t kMaxSamples = 180;
    QString title;
    QVector<QString> names;
    QVector<QColor> colors;
    QVector<double> seriesMax;
    QVector<double> adaptiveScale;
    std::vector<std::deque<double>> series;
};

PerformanceWidget::PerformanceWidget(QWidget *parent)
    : QWidget(parent)
{
    const QScreen *scr = QGuiApplication::primaryScreen();
    const qreal dpiScale = scr ? std::clamp(scr->logicalDotsPerInch() / 96.0, 1.0, 2.4) : 1.0;
    const int m = std::max(6, static_cast<int>(std::lround(8.0 * dpiScale)));

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(m, m, m, m);
    layout->setSpacing(std::max(4, static_cast<int>(std::lround(5.0 * dpiScale))));
    setMinimumHeight(460);
    setStyleSheet(QStringLiteral(
        "PerformanceWidget {\n"
        "  background-color:#1b1d20;\n"
        "  border-top:1px solid #343a42;\n"
        "}\n"
        "QLabel { color:#cfd6df; background-color:transparent; }"));

    auto *title = new QLabel("Performance / Utility", this);
    title->setStyleSheet(QStringLiteral("font-weight:600; color:#cfd6df; background:transparent;"));
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
    qualityGraph->setMinimumHeight(170);
    activityGraph->setMinimumHeight(170);

    layout->addWidget(title);
    layout->addWidget(composeFpsLabel);
    layout->addWidget(cacheStatsLabel);
    layout->addWidget(cacheResolutionLabel);
    layout->addWidget(projectInfoLabel);
    layout->addWidget(qualityGraph, 3);
    layout->addWidget(activityGraph, 3);
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
