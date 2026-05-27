#include "audiolevelmeterwidget.h"

#include <QFontDatabase>
#include <QFontMetrics>
#include <QPainter>
#include <QResizeEvent>

#include <algorithm>
#include <cmath>

namespace {
constexpr float kMeterMinDb = -48.f;
/// Meter scale tops at 0 dB; overload pegs at the 0 dB line (no +headroom band).
constexpr float kMeterMaxDb = 0.f;
constexpr float kSilenceDb = -60.f;
constexpr int kTimerMs = 25;
constexpr float kBarFallDbPerTick = 2.8f;
constexpr float kCapFallDbPerTick = 0.2f;
constexpr int kScaleW = 22;
constexpr int kChannelGap = 5;
constexpr int kMinBarW = 14;
constexpr int kSegPx = 6;
constexpr int kSegGap = 2;
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

    auto drawChannel = [&](const QRect &bar, float levelDb, float capDb, const char *chLabel) {
        if (bar.width() < 4 || bar.height() < 8) {
            return;
        }

        p.fillRect(bar, QColor(8, 9, 11));

        const int innerTop = bar.top() + 2;
        const int innerBottom = bar.bottom() - 2;
        const int innerH = std::max(1, innerBottom - innerTop);
        const int segCount = std::max(1, innerH / segStep);

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

            if (segDb <= levelDb + 0.35f) {
                const QColor col = segmentColor(segDb);
                p.fillRect(slot.adjusted(1, 1, -1, -1), col);
                p.setPen(QPen(col.lighter(125), 1));
                p.drawLine(slot.left() + 2, slot.top() + 1, slot.right() - 2, slot.top() + 1);
            }
        }

        if (capDb > levelDb + 0.4f && capDb > kMeterMinDb + 1.f) {
            const int yCap = dbToY(capDb, bar);
            QColor capCol = QColor(245, 242, 230);
            if (capDb >= -3.f) {
                capCol = QColor(255, 85, 70);
            } else if (capDb >= -9.f) {
                capCol = QColor(255, 205, 60);
            }
            const QRect capLine(bar.left() + 1, yCap - 1, bar.width() - 2, 3);
            p.fillRect(capLine, capCol);
        }

        p.setPen(QColor(210, 218, 228));
        p.setFont(labelFont);
        p.drawText(QRect(bar.left(), rect().bottom() - 15, bar.width(), 14), Qt::AlignHCenter | Qt::AlignVCenter,
                   QString::fromLatin1(chLabel));
    };

    drawChannel(barL, levelL, capHoldL, "L");
    drawChannel(barR, levelR, capHoldR, "R");
}
