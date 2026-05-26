#ifndef AUDIOLEVELMETERWIDGET_H
#define AUDIOLEVELMETERWIDGET_H

#include <QWidget>

/// Stereo peak/VU-style level meters (Sony Vegas–style), real-time.
class AudioLevelMeterWidget : public QWidget
{
    Q_OBJECT
public:
    explicit AudioLevelMeterWidget(QWidget *parent = nullptr);

    /// Peak levels in dBFS (−60 = silence, 0 = full scale, may exceed 0 on hot material).
    void pushLevelsDb(float leftDb, float rightDb);

protected:
    void paintEvent(QPaintEvent *event) override;
    void timerEvent(QTimerEvent *event) override;

private:
    static float decayDb(float current, float target);
    static QColor colorForDb(float db);

    float levelL = -60.f;
    float levelR = -60.f;
    float targetL = -60.f;
    float targetR = -60.f;
    float peakHoldL = -60.f;
    float peakHoldR = -60.f;
};

#endif
