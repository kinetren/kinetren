#ifndef PERFORMANCEWIDGET_H
#define PERFORMANCEWIDGET_H

#include <QWidget>

class QLabel;
class MetricGraphWidget;

class PerformanceWidget : public QWidget
{
    Q_OBJECT

public:
    explicit PerformanceWidget(QWidget *parent = nullptr);
    void setMetrics(const QString &composeFps,
                    const QString &cacheStats,
                    const QString &cacheResolution,
                    const QString &projectInfo,
                    double fpsValue,
                    double cacheHitPct,
                    double avcppCallsPerSecond);

private:
    QLabel *composeFpsLabel = nullptr;
    QLabel *cacheStatsLabel = nullptr;
    QLabel *cacheResolutionLabel = nullptr;
    QLabel *projectInfoLabel = nullptr;
    MetricGraphWidget *fpsGraph = nullptr;
    MetricGraphWidget *cacheHitGraph = nullptr;
    MetricGraphWidget *avcppGraph = nullptr;
};

#endif // PERFORMANCEWIDGET_H
