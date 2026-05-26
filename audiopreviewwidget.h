#ifndef AUDIOPREVIEWWIDGET_H
#define AUDIOPREVIEWWIDGET_H

#include <QVector>
#include <QWidget>

/// Stereo waveform strip (peak columns) with optional playhead marker.
class AudioPreviewWidget : public QWidget
{
    Q_OBJECT
public:
    explicit AudioPreviewWidget(QWidget *parent = nullptr);

    void setPeaks(const QVector<QPair<int, int>> &stereoColumns);
    /// 0..1 position along the waveform (cursor within clip), or negative to hide.
    void setPlayheadRel(double rel01);
    void setStatusText(const QString &text);

protected:
    void paintEvent(QPaintEvent *event) override;

private:
    QVector<QPair<int, int>> peaks;
    double playheadRel = -1.0;
    QString statusText;
};

#endif
