#ifndef PERFORMANCEWIDGET_H
#define PERFORMANCEWIDGET_H

#include <QWidget>
#include <QVector>

class QLabel;
class MultiLineGraphWidget;

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
                    double finalCacheHitPct,
                    double audioCacheHitPct,
                    double avcppCallsPerSecond,
                    double avcppOpenPerSec,
                    double avcppReadPerSec,
                    double avcppVideoDecodePerSec,
                    double avcppAudioDecodePerSec);

private:
    QLabel *composeFpsLabel = nullptr;
    QLabel *cacheStatsLabel = nullptr;
    QLabel *cacheResolutionLabel = nullptr;
    QLabel *projectInfoLabel = nullptr;
    MultiLineGraphWidget *qualityGraph = nullptr;
    MultiLineGraphWidget *activityGraph = nullptr;
    QLabel *qualityLegendLabel = nullptr;
    QLabel *activityLegendLabel = nullptr;
};

#endif // PERFORMANCEWIDGET_H
