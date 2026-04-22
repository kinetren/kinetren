#ifndef PREVIEWER_H
#define PREVIEWER_H

#include <QImage>
#include <QPixmap>
#include <QMouseEvent>
#include <QPaintEvent>
#include <QTimerEvent>
#include <QWidget>

#include <array>
#include <atomic>

class Previewer : public QWidget
{
    Q_OBJECT

    static constexpr int kFrameBufferCount = 20;

    enum class FrameState : int {
        Empty = 0,
        Writing = 1,
        Ready = 2,
        Painting = 3,
        Painted = 4
    };

    struct RPFrame {
        std::atomic_int state{static_cast<int>(FrameState::Empty)};
        uint64_t serial = 0;
        QImage img;
        QPixmap displayPixmap;
    };

public:
    explicit Previewer(QWidget *parent = nullptr);
    ~Previewer() override;

    void paintIt(QImage &&img);
    void setBuffering(bool enabled);
    void setSmoothScaling(bool enabled);
    void setAntialiasing(bool enabled);

protected:
    void timerEvent(QTimerEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void paintEvent(QPaintEvent *event) override;

private:
    void drawBufferingOverlay(QPainter &painter);

    std::array<RPFrame, kFrameBufferCount> frameBuffer{};
    int frameWriteIndex = 0;
    int frameReadIndex = 0;
    int previousPaintedIndex = -1;
    int paintTimer = -1;
    bool pause = false;
    bool buffering = false;
    bool smoothScaling = true;
    bool antialiasing = false;
    uint64_t nextFrameSerial = 1;
    std::atomic<uint64_t> lastPaintNs{0};
};

#endif // PREVIEWER_H
