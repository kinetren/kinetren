#ifndef PREVIEWER_H
#define PREVIEWER_H

#include <QImage>
#include <QMouseEvent>
#include <QPaintEvent>
#include <QTimerEvent>
#include <QWidget>

#include <array>
#include <atomic>
#include <condition_variable>
#include <mutex>

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
        QImage img;
    };

public:
    explicit Previewer(QWidget *parent = nullptr);
    ~Previewer() override;

    void paintIt(QImage &&img);

protected:
    void timerEvent(QTimerEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void paintEvent(QPaintEvent *event) override;

private:
    std::array<RPFrame, kFrameBufferCount> frameBuffer{};
    int frameWriteIndex = 0;
    int frameReadIndex = 0;
    int previousPaintedIndex = -1;
    int paintTimer = -1;
    bool pause = false;
    std::atomic<uint64_t> lastPaintNs{0};

    std::mutex mutex;
    std::condition_variable frameReady;
};

#endif // PREVIEWER_H
