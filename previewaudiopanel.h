#ifndef PREVIEWAUDIOPANEL_H
#define PREVIEWAUDIOPANEL_H

#include <QWidget>

class QLabel;

/// Audio meters + volume fader with a top-centered dB overlay (does not shift layout).
class PreviewAudioPanel : public QWidget
{
    Q_OBJECT
public:
    explicit PreviewAudioPanel(QWidget *parent = nullptr);

    QWidget *contentRow() const { return metersRow; }
    void setVolumeOverlayText(const QString &text);

protected:
    void resizeEvent(QResizeEvent *event) override;

private:
    void repositionOverlay();

    QWidget *metersRow = nullptr;
    QLabel *volumeOverlay = nullptr;
    int overlayBandHeight_ = 0;
};

#endif
