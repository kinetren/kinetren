#ifndef SPEEDCHARTWIDGET_H
#define SPEEDCHARTWIDGET_H

#include <QWidget>

class MultiLineGraphWidget;

class SpeedChartWidget : public QWidget
{
    Q_OBJECT

public:
    explicit SpeedChartWidget(QWidget *parent = nullptr);

    void addSample(double speedRealtimeFactor);
    void clear();

    QSize sizeHint() const override;
    QSize minimumSizeHint() const override;

private:
    MultiLineGraphWidget *m_graph = nullptr;
};

#endif
