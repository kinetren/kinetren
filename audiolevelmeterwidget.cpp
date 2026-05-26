#include "audiolevelmeterwidget.h"

#include <QPainter>
#include <QLinearGradient>
#include <QTimerEvent>

#include <algorithm>
#include <cmath>

namespace {
constexpr float kMeterMinDb = -54.f;
constexpr float kMeterMaxDb = 6.f;
constexpr float kSilenceDb = -60.f;
constexpr int kDecayTimerMs = 33;

float clampDb(float db)
{
    return std::clamp(db, kSilenceDb, kMeterMaxDb + 6.f);
}
} // namespace

AudioLevelMeterWidget::AudioLevelMeterWidget(QWidget *parent)
    : QWidget(parent)
{
    setMinimumWidth(52);
    setMaximumWidth(72);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    startTimer(kDecayTimerMs);
}

void AudioLevelMeterWidget::pushLevelsDb(float leftDb, float rightDb)
{
    targetL = clampDb(leftDb);
    targetR = clampDb(rightDb);
    levelL = std::max(levelL, targetL);
    levelR = std::max(levelR, targetR);
    if (targetL > peakHoldL) {
        peakHoldL = targetL;
    }
    if (targetR > peakHoldR) {
        peakHoldR = targetR;
    }
    update();
}

float AudioLevelMeterWidget::decayDb(float current, float target)
{
    if (target > current) {
        return target;
    }
    return current - 2.8f;
}

QColor AudioLevelMeterWidget::colorForDb(float db)
{
    if (db >= -3.f) {
        return QColor(235, 75, 65);
    }
    if (db >= -9.f) {
        return QColor(240, 190, 55);
    }
    if (db >= -18.f) {
        return QColor(120, 210, 95);
    }
    return QColor(70, 160, 88);
}

void AudioLevelMeterWidget::timerEvent(QTimerEvent *event)
{
    Q_UNUSED(event);
    levelL = decayDb(levelL, targetL);
    levelR = decayDb(levelR, targetR);
    targetL = decayDb(targetL, kSilenceDb);
    targetR = decayDb(targetR, kSilenceDb);
    peakHoldL = decayDb(peakHoldL, kSilenceDb);
    peakHoldR = decayDb(peakHoldR, kSilenceDb);
    update();
}

void AudioLevelMeterWidget::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.fillRect(rect(), QColor(18, 20, 22));

    const QRect area = rect().adjusted(3, 4, -3, -18);
    const int scaleW = 18;
    const int gap = 4;
    const int barW = std::max(10, (area.width() - scaleW - gap * 2) / 2);
    const QRect scaleRect(area.left(), area.top(), scaleW, area.height());
    const QRect barL(scaleRect.right() + gap, area.top(), barW, area.height());
    const QRect barR(barL.right() + gap, area.top(), barW, area.height());

    auto dbToY = [&](float db) {
        const float t = (clampDb(db) - kMeterMinDb) / (kMeterMaxDb - kMeterMinDb);
        return barL.bottom() - static_cast<int>(std::llround(std::clamp(t, 0.f, 1.f) * barL.height()));
    };

    p.setPen(QColor(95, 105, 118));
    p.setFont(QFont(QStringLiteral("Sans"), 7));
    for (float db : {0.f, -6.f, -12.f, -24.f, -48.f}) {
        const int y = dbToY(db);
        p.drawLine(scaleRect.right() - 2, y, scaleRect.right(), y);
        if (db == 0.f || db == -12.f || db == -48.f) {
            p.drawText(QRect(scaleRect.left(), y - 6, scaleW - 3, 12), Qt::AlignRight | Qt::AlignVCenter,
                       db > -0.1f ? QStringLiteral("0") : QString::number(static_cast<int>(db)));
        }
    }

    auto drawBar = [&](const QRect &bar, float levelDb, float peakDb, const QString &label) {
        p.fillRect(bar, QColor(28, 32, 36));
        p.setPen(QColor(48, 54, 62));
        p.drawRect(bar);

        const int yLevel = dbToY(levelDb);
        const int yTop = bar.top() + 1;
        if (yLevel < bar.bottom() - 1) {
            QRect fill(bar.left() + 1, yLevel, bar.width() - 2, bar.bottom() - 1 - yLevel);
            QLinearGradient grad(fill.topLeft(), fill.bottomLeft());
            grad.setColorAt(0.0, colorForDb(levelDb));
            grad.setColorAt(1.0, colorForDb(kMeterMinDb));
            p.fillRect(fill, grad);
        }

        const int yPeak = dbToY(peakDb);
        if (peakDb > kMeterMinDb + 1.f) {
            p.setPen(QPen(colorForDb(peakDb), 2));
            p.drawLine(bar.left() + 1, yPeak, bar.right() - 1, yPeak);
        }

        p.setPen(QColor(170, 180, 195));
        p.setFont(QFont(QStringLiteral("Sans"), 8, QFont::Bold));
        p.drawText(QRect(bar.left(), rect().bottom() - 15, bar.width(), 14), Qt::AlignHCenter, label);
    };

    drawBar(barL, levelL, peakHoldL, QStringLiteral("L"));
    drawBar(barR, levelR, peakHoldR, QStringLiteral("R"));

    p.setPen(QColor(50, 56, 64));
    p.drawRect(rect().adjusted(0, 0, -1, -1));
}
