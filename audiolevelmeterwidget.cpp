#include "audiolevelmeterwidget.h"

#include <QFontDatabase>
#include <QFontMetrics>
#include <QPainter>
#include <QRadialGradient>
#include <QResizeEvent>

#include <algorithm>
#include <cmath>
#include <utility>

namespace {
constexpr float kMeterMinDb = -48.f;
constexpr float kMeterMaxDb = 6.f;       // overload headroom above 0 dB
constexpr float kMeterNominalTopDb = 0.f; // red/yellow/green ladder ends here
constexpr float kSilenceDb = -60.f;
constexpr int kTimerMs = 25;
constexpr float kBarFallDbPerTick = 2.8f;
constexpr float kCapFallDbPerTick = 0.2f;
constexpr int kScaleW = 22;
constexpr int kChannelGap = 5;
constexpr int kMinBarW = 14;
constexpr int kSegPx = 6;
constexpr int kSegGap = 2;

QColor glowColor(float db, int alpha)
{
    QColor color;
    if (db > kMeterNominalTopDb) {
        const float t = std::clamp(db / kMeterMaxDb, 0.f, 1.f);
        color = QColor(255, static_cast<int>(88 - 28 * t), static_cast<int>(72 - 40 * t));
    } else if (db >= -3.f) {
        color = QColor(228, 62, 52);
    } else if (db >= -9.f) {
        color = QColor(238, 178, 42);
    } else if (db >= -18.f) {
        color = QColor(108, 198, 82);
    } else if (db >= -36.f) {
        color = QColor(58, 142, 68);
    } else {
        color = QColor(38, 98, 48);
    }
    color.setAlpha(alpha);
    return color;
}

void drawGlowEllipse(QPainter &p, const QPointF &center, qreal radiusX, qreal radiusY, QColor color)
{
    if (radiusX <= 0.0 || radiusY <= 0.0 || color.alpha() <= 0) {
        return;
    }

    QColor core = color;
    QColor soft = color;
    QColor clear = color;
    soft.setAlpha(color.alpha() / 3);
    clear.setAlpha(0);

    p.save();
    p.translate(center);
    p.scale(1.0, radiusY / radiusX);
    p.setPen(Qt::NoPen);
    QRadialGradient glow(QPointF(0.0, 0.0), radiusX);
    glow.setColorAt(0.0, core);
    glow.setColorAt(0.28, soft);
    glow.setColorAt(1.0, clear);
    p.setBrush(glow);
    p.drawEllipse(QPointF(0.0, 0.0), radiusX, radiusX);
    p.restore();
}

QColor capGlowColor(float db, int alpha)
{
    QColor color = QColor(245, 242, 230);
    if (db > kMeterNominalTopDb + 0.05f) {
        color = QColor(255, 120, 95);
    } else if (db >= -3.f) {
        color = QColor(255, 85, 70);
    } else if (db >= -9.f) {
        color = QColor(255, 205, 60);
    } else if (db < -18.f) {
        color = glowColor(db, alpha);
        return color;
    }
    color.setAlpha(alpha);
    return color;
}
} // namespace

AudioLevelMeterWidget::AudioLevelMeterWidget(QWidget *parent)
    : QWidget(parent)
{
    setMinimumWidth(104);
    setSizePolicy(QSizePolicy::MinimumExpanding, QSizePolicy::Expanding);
    QFont f = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    if (f.pointSize() <= 0) {
        f = font();
    }
    f.setPointSize(8);
    f.setStyleStrategy(QFont::PreferAntialias);
    setFont(f);
    startTimer(kTimerMs);
}

QSize AudioLevelMeterWidget::sizeHint() const
{
    return QSize(112, 220);
}

QSize AudioLevelMeterWidget::minimumSizeHint() const
{
    return QSize(104, 120);
}

void AudioLevelMeterWidget::setCenterWidget(QWidget *widget)
{
    if (centerWidget_ == widget) {
        return;
    }
    if (centerWidget_) {
        centerWidget_->setParent(nullptr);
    }
    centerWidget_ = widget;
    if (centerWidget_) {
        centerWidget_->setParent(this);
        centerWidget_->show();
        centerWidget_->raise();
        layoutCenterWidget();
    }
}

QRect AudioLevelMeterWidget::meterAreaRect() const
{
    return rect().adjusted(3, 4, -3, -18);
}

void AudioLevelMeterWidget::layoutCenterWidget()
{
    if (!centerWidget_) {
        return;
    }
    const QRect area = meterAreaRect();
    const int availForBars = std::max(0, area.width() - kScaleW - kChannelGap);
    const int barW = std::max(kMinBarW, (availForBars - kChannelGap) / 2);
    const QRect scaleRect(area.left(), area.top(), kScaleW, area.height());
    const QRect barL(scaleRect.right() + 1, area.top(), barW, area.height());
    const QRect barR(barL.right() + kChannelGap, area.top(), barW, area.height());
    const int faderW = barR.right() - barL.left();
    centerWidget_->setGeometry(barL.left(), 0, faderW, height());
    centerWidget_->raise();
}

void AudioLevelMeterWidget::resizeEvent(QResizeEvent *event)
{
    QWidget::resizeEvent(event);
    layoutCenterWidget();
}

float AudioLevelMeterWidget::clampDb(float db)
{
    return std::clamp(db, kSilenceDb, kMeterMaxDb);
}

float AudioLevelMeterWidget::decayToward(float current, float target, float fallDb)
{
    if (target > current) {
        return target;
    }
    return current - fallDb;
}

QColor AudioLevelMeterWidget::segmentColor(float segDb)
{
    if (segDb > kMeterNominalTopDb) {
        const float t = std::clamp(segDb / kMeterMaxDb, 0.f, 1.f);
        return QColor(255, static_cast<int>(88 - 28 * t), static_cast<int>(72 - 40 * t));
    }
    if (segDb >= -3.f) {
        return QColor(228, 62, 52);
    }
    if (segDb >= -9.f) {
        return QColor(238, 178, 42);
    }
    if (segDb >= -18.f) {
        return QColor(108, 198, 82);
    }
    if (segDb >= -36.f) {
        return QColor(58, 142, 68);
    }
    return QColor(38, 98, 48);
}

void AudioLevelMeterWidget::pushLevelsDb(float leftDb, float rightDb)
{
    targetL = clampDb(leftDb);
    targetR = clampDb(rightDb);
    if (targetL > levelL) {
        levelL = targetL;
    }
    if (targetR > levelR) {
        levelR = targetR;
    }
    if (targetL > capHoldL) {
        capHoldL = targetL;
    }
    if (targetR > capHoldR) {
        capHoldR = targetR;
    }
    update();
}

void AudioLevelMeterWidget::timerEvent(QTimerEvent *event)
{
    Q_UNUSED(event);
    levelL = decayToward(levelL, targetL, kBarFallDbPerTick);
    levelR = decayToward(levelR, targetR, kBarFallDbPerTick);
    targetL = decayToward(targetL, kSilenceDb, kBarFallDbPerTick * 0.7f);
    targetR = decayToward(targetR, kSilenceDb, kBarFallDbPerTick * 0.7f);
    capHoldL = decayToward(capHoldL, kSilenceDb, kCapFallDbPerTick);
    capHoldR = decayToward(capHoldR, kSilenceDb, kCapFallDbPerTick);
    update();
}

void AudioLevelMeterWidget::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, false);
    p.setRenderHint(QPainter::TextAntialiasing, true);

    p.fillRect(rect(), QColor(14, 16, 18));

    const QFont labelFont = font();
    const QFontMetrics fm(labelFont);

    const QRect area = meterAreaRect();
    const int availForBars = std::max(0, area.width() - kScaleW - kChannelGap);
    const int barW = std::max(kMinBarW, (availForBars - kChannelGap) / 2);

    const QRect scaleRect(area.left(), area.top(), kScaleW, area.height());
    const QRect barL(scaleRect.right() + 1, area.top(), barW, area.height());
    const QRect barR(barL.right() + kChannelGap, area.top(), barW, area.height());

    auto dbToY = [&](float db, const QRect &bar) {
        const float t = (clampDb(db) - kMeterMinDb) / (kMeterMaxDb - kMeterMinDb);
        return bar.bottom() - static_cast<int>(std::llround(std::clamp(t, 0.f, 1.f) * bar.height()));
    };

    p.setFont(labelFont);
    p.setPen(QColor(175, 185, 198));
    for (float db : {0.f, -12.f, -24.f, -48.f}) {
        const int y = dbToY(db, barL);
        p.drawLine(scaleRect.right() - 2, y, scaleRect.right(), y);
        const QString txt = (db > -0.05f) ? QStringLiteral("0") : QString::number(static_cast<int>(db));
        const QRect tr(scaleRect.left() - 2, y - fm.height() / 2, kScaleW + 2, fm.height());
        p.drawText(tr, Qt::AlignRight | Qt::AlignVCenter, txt);
    }

    const int segStep = kSegPx + kSegGap;
    auto segmentSlotForDb = [&](float db, const QRect &bar) {
        const int innerTop = bar.top() + 2;
        const int innerBottom = bar.bottom() - 2;
        const int innerH = std::max(1, innerBottom - innerTop);
        const int segCount = std::max(1, innerH / segStep);
        const int rawY = dbToY(db, bar);
        return std::clamp(static_cast<int>(std::llround(static_cast<float>(innerBottom - rawY) /
                                                        static_cast<float>(segStep))),
                          0, segCount - 1);
    };
    auto segmentCenterYForDb = [&](float db, const QRect &bar) {
        const int innerBottom = bar.bottom() - 2;
        const int segBottom = innerBottom - segmentSlotForDb(db, bar) * segStep;
        const int segTop = segBottom - kSegPx;
        return segTop + kSegPx / 2;
    };

    auto drawChannel = [&](const QRect &bar, float levelDb, float capDb, const char *chLabel) {
        if (bar.width() < 4 || bar.height() < 8) {
            return;
        }

        p.fillRect(bar, QColor(8, 9, 11));

        const int innerTop = bar.top() + 2;
        const int innerBottom = bar.bottom() - 2;
        const int innerH = std::max(1, innerBottom - innerTop);
        const int segCount = std::max(1, innerH / segStep);
        const bool hasCap = capDb > levelDb + 0.4f && capDb > kMeterMinDb + 1.f;
        const int capSlot = hasCap ? segmentSlotForDb(capDb, bar) : -1;

        for (int s = 0; s < segCount; ++s) {
            const int segBottom = innerBottom - s * segStep;
            const int segTop = segBottom - kSegPx;
            if (segTop < innerTop) {
                break;
            }

            const float relCenter =
                1.f - static_cast<float>(segTop + kSegPx / 2 - innerTop) / static_cast<float>(innerH);
            const float segDb = kMeterMinDb + relCenter * (kMeterMaxDb - kMeterMinDb);

            const QRect slot(bar.left() + 2, segTop, bar.width() - 4, kSegPx);
            p.fillRect(slot, QColor(14, 16, 19));

            if (segDb <= levelDb + 0.35f || s == capSlot) {
                const QColor col = segmentColor(segDb);
                p.fillRect(slot.adjusted(1, 1, -1, -1), col);
                p.setPen(QPen(col.lighter(125), 1));
                p.drawLine(slot.left() + 2, slot.top() + 1, slot.right() - 2, slot.top() + 1);
            }
        }

        const int yZero = dbToY(kMeterNominalTopDb, bar);
        p.setPen(QPen(QColor(95, 105, 118, 140), 1));
        p.drawLine(bar.left() + 1, yZero, bar.right() - 1, yZero);

        p.setPen(QColor(210, 218, 228));
        p.setFont(labelFont);
        p.drawText(QRect(bar.left(), rect().bottom() - 15, bar.width(), 14), Qt::AlignHCenter | Qt::AlignVCenter,
                   QString::fromLatin1(chLabel));
    };

    drawChannel(barL, levelL, capHoldL, "L");
    drawChannel(barR, levelR, capHoldR, "R");

    const float glowLevelDb = std::max(levelL, levelR);
    if (glowLevelDb > kMeterMinDb + 0.5f) {
        const int glowTop = dbToY(glowLevelDb, barL);
        const int innerTop = barL.top() + 2;
        const int innerBottom = barL.bottom() - 2;
        const int innerH = std::max(1, innerBottom - innerTop);
        const qreal glowX = (barL.left() + barR.right()) * 0.5;
        const qreal wideRadius = std::max<qreal>(area.width() * 1.15, barR.right() - scaleRect.left() + 28);

        p.save();
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setCompositionMode(QPainter::CompositionMode_Plus);
        p.setPen(Qt::NoPen);

        for (int y = innerBottom; y >= glowTop; y -= kSegPx + kSegGap) {
            const float rel = 1.f - static_cast<float>(y - innerTop) / static_cast<float>(innerH);
            const float db = kMeterMinDb + rel * (kMeterMaxDb - kMeterMinDb);
            drawGlowEllipse(p, QPointF(glowX, y), wideRadius * 0.7, 18.0, glowColor(db, 42));
        }

        for (const auto &[topDb, bottomDb] :
             {std::pair<float, float>{kMeterMaxDb, 0.f}, {0.f, -3.f}, {-3.f, -9.f}, {-9.f, -18.f},
              {-18.f, -36.f}, {-36.f, kMeterMinDb}}) {
            const float activeTopDb = std::min(glowLevelDb, topDb);
            if (activeTopDb <= bottomDb) {
                continue;
            }

            const int zoneTop = dbToY(activeTopDb, barL);
            const int zoneBottom = dbToY(bottomDb, barL);
            const qreal zoneCenterY = (zoneTop + zoneBottom) * 0.5;
            const qreal zoneRadiusY = std::max<qreal>(32.0, (zoneBottom - zoneTop) * 0.78 + 24.0);
            drawGlowEllipse(p, QPointF(glowX, zoneCenterY), wideRadius, zoneRadiusY,
                            glowColor((activeTopDb + bottomDb) * 0.5f, 24));
        }

        drawGlowEllipse(p, QPointF(glowX, glowTop), wideRadius * 1.08, 38.0, glowColor(glowLevelDb, 42));
        drawGlowEllipse(p, QPointF(glowX, glowTop - 14), wideRadius * 0.95, 34.0, glowColor(glowLevelDb, 28));
        drawGlowEllipse(p, QPointF(glowX, rect().bottom() - 26), wideRadius * 0.85, 28.0,
                        glowColor(std::min(glowLevelDb, -18.f), 20));
        p.restore();
    }

    auto drawCapGlow = [&](const QRect &bar, float levelDb, float capDb) {
        if (capDb <= levelDb + 0.4f || capDb <= kMeterMinDb + 1.f) {
            return;
        }

        const int yCap = segmentCenterYForDb(capDb, bar);
        p.save();
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setCompositionMode(QPainter::CompositionMode_Plus);
        p.setPen(Qt::NoPen);
        drawGlowEllipse(p, QPointF(bar.center().x(), yCap), bar.width() * 2.7, 20.0, capGlowColor(capDb, 52));
        drawGlowEllipse(p, QPointF(bar.center().x(), yCap), bar.width() * 4.6, 38.0, capGlowColor(capDb, 26));
        drawGlowEllipse(p, QPointF(bar.center().x(), yCap - 8), bar.width() * 3.8, 28.0, capGlowColor(capDb, 18));
        p.restore();
    };

    drawCapGlow(barL, levelL, capHoldL);
    drawCapGlow(barR, levelR, capHoldR);
}
