#include "previewer.h"

#include <QPainter>
#include <QFont>
#include <qdebug.h>

#include <chrono>
#include <cstdlib>
#include <limits>
#include <utility>

Previewer::Previewer(QWidget *parent)
    : QWidget(parent)
{
}

Previewer::~Previewer() = default;

void Previewer::mousePressEvent(QMouseEvent *event)
{
    pause = !pause;
    QWidget::mousePressEvent(event);
}

void Previewer::submitFrame(QImage image)
{
    paintIt(std::move(image));
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
        // All slots are holding undrawn frames: replace the oldest pending frame so we always
        // keep the latest image (otherwise playback can stall on the first N frames forever).
        uint64_t oldestSerial = std::numeric_limits<uint64_t>::max();
        for (int i = 0; i < kFrameBufferCount; ++i) {
            if (frameBuffer[i].state != static_cast<int>(FrameState::Ready)) {
                continue;
            }
            if (frameBuffer[i].serial < oldestSerial) {
                oldestSerial = frameBuffer[i].serial;
                bufferIndex = i;
            }
        }
    }

    if (bufferIndex < 0) {
        return;
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Writing);
    frameBuffer[bufferIndex].img = std::move(img);
    frameBuffer[bufferIndex].serial = nextFrameSerial++;
    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Ready);
    update();
}

void Previewer::setBuffering(bool enabled)
{
    buffering = enabled;
    update();
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

void Previewer::drawBufferingOverlay(QPainter &painter)
{
    painter.fillRect(rect(), QColor(0, 0, 0, 140));
    QFont f = painter.font();
    f.setPointSize(std::max(11, f.pointSize()));
    painter.setFont(f);
    painter.setPen(QColor(230, 235, 245));
    painter.drawText(rect(), Qt::AlignCenter, QStringLiteral("Загрузка…"));
}

void Previewer::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);

    int bufferIndex = -1;
    if (pause) {
        if (previousPaintedIndex < 0) {
            if (buffering) {
                QPainter p(this);
                if (p.isActive()) {
                    p.fillRect(rect(), Qt::black);
                    drawBufferingOverlay(p);
                }
            }
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
                if (buffering) {
                    QPainter p(this);
                    if (p.isActive()) {
                        p.fillRect(rect(), Qt::black);
                        drawBufferingOverlay(p);
                    }
                }
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
        const auto &slot = frameBuffer[bufferIndex];
        if (!slot.img.isNull()) {
            const QRect widgetRect = rect();
            const QSize scaled =
                slot.img.size().scaled(widgetRect.size(), Qt::KeepAspectRatio);
            QRect target(QPoint(0, 0), scaled);
            target.moveCenter(widgetRect.center());
            painter.drawImage(QRectF(target), slot.img, QRectF(slot.img.rect()));
        }
        if (buffering) {
            drawBufferingOverlay(painter);
        }
    } else {
        qCritical() << "QPainter not active";
    }

    frameBuffer[bufferIndex].state = static_cast<int>(FrameState::Painted);
    previousPaintedIndex = bufferIndex;
    frameReadIndex = (bufferIndex + 1) % kFrameBufferCount;
}
