#include "previewer.h"

#include <QPainter>
#include <QPixmap>
#include <qdebug.h>

#include <chrono>
#include <cstdlib>
#include <utility>

Previewer::Previewer(QWidget *parent)
    : QWidget(parent)
{
    using namespace std::chrono_literals;
    paintTimer = startTimer(24ms, Qt::VeryCoarseTimer);
    if (paintTimer < 0) {
        abort();
    }
}

Previewer::~Previewer()
{
    if (paintTimer >= 0) {
        killTimer(paintTimer);
    }
}

void Previewer::timerEvent(QTimerEvent *event)
{
    if (event->timerId() != paintTimer) {
        return;
    }

    for (int i = 0; i < kFrameBufferCount; ++i) {
        if (frameBuffer[i].state == static_cast<int>(FrameState::Ready)) {
            update();
            return;
        }
    }
}

void Previewer::mousePressEvent(QMouseEvent *event)
{
    pause = !pause;
    QWidget::mousePressEvent(event);
}

void Previewer::paintIt(QImage &&img)
{
    int bufferIndex = -1;
    for (int i = 0; i < kFrameBufferCount; ++i) {
        const int candidate = (frameWriteIndex + i) % kFrameBufferCount;
        const int state = frameBuffer[candidate].state;
        if (state == static_cast<int>(FrameState::Empty) || state == static_cast<int>(FrameState::Painted)) {
            bufferIndex = candidate;
            frameWriteIndex = candidate + 1;
            break;
        }
    }

    if (bufferIndex < 0) {
        // UI is behind decode speed; drop this frame to keep the event loop responsive.
        return;
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Writing);
    frameBuffer[bufferIndex].displayPixmap = QPixmap();
    frameBuffer[bufferIndex].img = std::move(img);
    frameBuffer[bufferIndex].serial = nextFrameSerial++;
    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Ready);
}

void Previewer::setSmoothScaling(bool enabled)
{
    smoothScaling = enabled;
    update();
}

void Previewer::setAntialiasing(bool enabled)
{
    antialiasing = enabled;
    update();
}

void Previewer::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);

    int bufferIndex = -1;
    if (pause) {
        if (previousPaintedIndex < 0) {
            return;
        }
        bufferIndex = previousPaintedIndex;
    } else {
        uint64_t bestSerial = 0;
        for (int i = 0; i < kFrameBufferCount; ++i) {
            if (frameBuffer[i].state != static_cast<int>(FrameState::Ready)) {
                continue;
            }
            if (frameBuffer[i].serial >= bestSerial) {
                bestSerial = frameBuffer[i].serial;
                bufferIndex = i;
            }
        }
        if (bufferIndex < 0) {
            if (previousPaintedIndex >= 0) {
                bufferIndex = previousPaintedIndex;
            } else {
                return;
            }
        }
        if (previousPaintedIndex >= 0 && previousPaintedIndex != bufferIndex) {
            frameBuffer[previousPaintedIndex].state = static_cast<int>(FrameState::Empty);
        }
        lastPaintNs = std::chrono::high_resolution_clock::now().time_since_epoch().count();
    }

    const int frameState = frameBuffer[bufferIndex].state;
    if (frameState != static_cast<int>(FrameState::Ready) &&
        frameState != static_cast<int>(FrameState::Painted)) {
        return;
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Painting);
    QPainter painter(this);
    if (painter.isActive()) {
        painter.fillRect(rect(), Qt::black);
        painter.setRenderHint(QPainter::Antialiasing, antialiasing);
        painter.setRenderHint(QPainter::SmoothPixmapTransform, smoothScaling);
        auto &slot = frameBuffer[bufferIndex];
        if (slot.displayPixmap.isNull() && !slot.img.isNull()) {
            slot.displayPixmap = QPixmap::fromImage(slot.img, Qt::ColorOnly);
        }
        if (!slot.displayPixmap.isNull()) {
            painter.drawPixmap(0, 0, width(), height(), slot.displayPixmap);
        }
    } else {
        qCritical() << "QPainter not active";
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Painted);
    previousPaintedIndex = bufferIndex;
    frameReadIndex = (bufferIndex + 1) % kFrameBufferCount;
}
