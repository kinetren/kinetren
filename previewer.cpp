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
    frameBuffer[bufferIndex].img = std::move(img);
    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Ready);
}

void Previewer::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);

    int bufferIndex = frameReadIndex % kFrameBufferCount;

    bool shouldPaintPrevious = false;
    const auto nowNs = std::chrono::high_resolution_clock::now().time_since_epoch().count();
    if (lastPaintNs > 0) {
        const auto diff = nowNs - lastPaintNs.load();
        if (diff < 25'000'000) {
            shouldPaintPrevious = true;
        }
    }

    if (pause) {
        shouldPaintPrevious = true;
    }

    if (shouldPaintPrevious) {
        if (previousPaintedIndex >= 0) {
            bufferIndex = previousPaintedIndex;
        } else {
            previousPaintedIndex = 0;
            frameReadIndex = 0;
            bufferIndex = 0;
        }
    } else {
        if (previousPaintedIndex >= 0 && previousPaintedIndex != bufferIndex) {
            frameBuffer[previousPaintedIndex].state = static_cast<int>(FrameState::Empty);
        }
        lastPaintNs = nowNs;
    }

    const int frameState = frameBuffer[bufferIndex].state;
    if (frameState != static_cast<int>(FrameState::Ready) &&
        frameState != static_cast<int>(FrameState::Painted)) {
        return;
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Painting);
    QPainter painter(this);
    if (painter.isActive()) {
        QPixmap pixmap = QPixmap::fromImage(frameBuffer[bufferIndex].img, Qt::ColorOnly);
        painter.drawPixmap(0, 0, width(), height(), pixmap);
    } else {
        qCritical() << "QPainter not active";
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Painted);
    previousPaintedIndex = bufferIndex;
    frameReadIndex = bufferIndex + 1;
}
