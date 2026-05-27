#ifndef AUDIOLEVELMETERWIDGET_H
#define AUDIOLEVELMETERWIDGET_H

#include <QWidget>

/// Stereo broadcast-style peak meters with Vegas-like peak-hold caps.
class AudioLevelMeterWidget : public QWidget
{
    Q_OBJECT
public:
    explicit AudioLevelMeterWidget(QWidget *parent = nullptr);

    void pushLevelsDb(float leftDb, float rightDb);

    /// Vertical fader slot between L/R bars (e.g. PreviewVolumeFader).
    void setCenterWidget(QWidget *widget);

    QSize sizeHint() const override;
    QSize minimumSizeHint() const override;

protected:
    void paintEvent(QPaintEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;
    void timerEvent(QTimerEvent *event) override;

private:
    static float clampDb(float db);
    static float decayToward(float current, float target, float fallDb);
    static QColor segmentColor(float segTopDb);
    void layoutCenterWidget();
    QRect meterAreaRect() const;

    QWidget *centerWidget_ = nullptr;
    float levelL = -60.f;
    float levelR = -60.f;
    float targetL = -60.f;
    float targetR = -60.f;
    /// Peak-hold “cap” (крышечка) — slow decay, shows extrema.
    float capHoldL = -60.f;
    float capHoldR = -60.f;
};

#endif
