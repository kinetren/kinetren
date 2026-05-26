#ifndef PREVIEWVOLUMEFADER_H
#define PREVIEWVOLUMEFADER_H

#include <QWidget>

/// Vertical master preview fader (Sony Vegas–style): mute, drag — dB readout lives in parent panel.
class PreviewVolumeFader : public QWidget
{
    Q_OBJECT
public:
    explicit PreviewVolumeFader(QWidget *parent = nullptr);

    void setVolumeDb(float db);
    float volumeDb() const { return volumeDb_; }
    void setMuted(bool muted);
    bool isMuted() const { return muted_; }
    bool isDragging() const { return dragging_; }

signals:
    void volumeChanged(float linearGain);
    void volumeDbChanged(float db);
    void activityChanged(bool active);

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void mouseDoubleClickEvent(QMouseEvent *event) override;
    QSize sizeHint() const override;
    QSize minimumSizeHint() const override;

private:
    void applyFromY(int y);
    void emitVolume();
    void setActivity(bool active);
    static float dbToLinear(float db);
    static float linearToDb(float linear);
    int yForDb(float db) const;
    float dbForY(int y) const;
    QRect railRect() const;
    QRect muteButtonRect() const;

    float volumeDb_ = 0.f;
    bool muted_ = false;
    bool dragging_ = false;
};

#endif
