#ifndef PREVIEWVOLUMEFADER_H
#define PREVIEWVOLUMEFADER_H

#include <QWidget>

/// Vertical master preview fader (Sony Vegas–style): dB scale, mute, drag.
class PreviewVolumeFader : public QWidget
{
    Q_OBJECT
public:
    explicit PreviewVolumeFader(QWidget *parent = nullptr);

    void setVolumeDb(float db);
    float volumeDb() const { return volumeDb_; }
    void setMuted(bool muted);
    bool isMuted() const { return muted_; }

signals:
    void volumeChanged(float linearGain);
    void volumeDbChanged(float db);

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
    static float dbToLinear(float db);
    static float linearToDb(float linear);
    int yForDb(float db) const;
    float dbForY(int y) const;
    QRect railRect() const;

    float volumeDb_ = 0.f;
    bool muted_ = false;
    bool dragging_ = false;
};

#endif
