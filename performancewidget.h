#ifndef PERFORMANCEWIDGET_H
#define PERFORMANCEWIDGET_H

#include <QWidget>

class QLabel;
class PlaybackPipeline;
class ProjectSettings;

class PerformanceWidget : public QWidget
{
    Q_OBJECT

public:
    explicit PerformanceWidget(QWidget *parent = nullptr);
    void setMetrics(const QString &composeFps,
                    const QString &cacheStats,
                    const QString &cacheResolution,
                    const QString &projectInfo);

private:
    QLabel *composeFpsLabel = nullptr;
    QLabel *cacheStatsLabel = nullptr;
    QLabel *cacheResolutionLabel = nullptr;
    QLabel *projectInfoLabel = nullptr;
};

#endif // PERFORMANCEWIDGET_H
