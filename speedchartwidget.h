#ifndef SPEEDCHARTWIDGET_H
#define SPEEDCHARTWIDGET_H

#include <QWidget>

#include <QVector>

class SpeedChartWidget : public QWidget
{
    Q_OBJECT

public:
    explicit SpeedChartWidget(QWidget *parent = nullptr);

    void addSample(double speedRealtimeFactor);
    void clear();

protected:
    void paintEvent(QPaintEvent *event) override;
    QSize sizeHint() const override;
    QSize minimumSizeHint() const override;

private:
    QVector<double> m_samples;
    static constexpr int kMaxSamples = 480;
};

#endif
