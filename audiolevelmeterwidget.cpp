#include "audiolevelmeterwidget.h"

#include <QFontMetrics>
#include <QPainter>
#include <QFontDatabase>

#include <algorithm>
#include <cmath>

namespace {
constexpr float kMeterMinDb = -48.f;
constexpr float kMeterMaxDb = 6.f;
constexpr float kSilenceDb = -60.f;
constexpr int kTimerMs = 25;
constexpr float kBarFallDbPerTick = 2.4f;
constexpr float kCapFallDbPerTick = 0.22f;
constexpr int kSegPx = 3;
constexpr int kSegGap = 1;
} // namespace

AudioLevelMeterWidget::AudioLevelMeterWidget(QWidget *parent)
    : QWidget(parent)
{
    setMinimumWidth(68);
    setMaximumWidth(88);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    QFont f = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    if (f.pointSize() <= 0) {
        f = font();
    }
    f.setPointSize(9);
    f.setStyleStrategy(QFont::PreferAntialias);
    setFont(f);
    startTimer(kTimerMs);
}

float AudioLevelMeterWidget::clampDb(float db)
{
    return std::clamp(db, kSilenceDb, kMeterMaxDb + 3.f);
}

float AudioLevelMeterWidget::decayToward(float current, float target, float fallDb)
{
    if (target >= current) {
        return target;
    }
    return current - fallDb;
}

QColor AudioLevelMeterWidget::segmentColor(float segTopDb)
{
    if (segTopDb >= -3.f) {
        return QColor(228, 62, 52);
    }
    if (segTopDb >= -9.f) {
        return QColor(238, 178, 42);
    }
    if (segTopDb >= -18.f) {
        return QColor(108, 198, 82);
    }
    if (segTopDb >= -36.f) {
        return QColor(58, 142, 68);
    }
    return QColor(32, 72, 42);
}

void AudioLevelMeterWidget::pushLevelsDb(float leftDb, float rightDb)
{
    targetL = clampDb(leftDb);
    targetR = clampDb(rightDb);
    levelL = std::max(levelL, targetL);
    levelR = std::max(levelR, targetR);
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
    targetL = decayToward(targetL, kSilenceDb, kBarFallDbPerTick * 0.65f);
    targetR = decayToward(targetR, kSilenceDb, kBarFallDbPerTick * 0.65f);
    capHoldL = decayToward(capHoldL, kSilenceDb, kCapFallDbPerTick);
    capHoldR = decayToward(capHoldR, kSilenceDb, kCapFallDbPerTick);
    update();
}

void AudioLevelMeterWidget::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setRenderHint(QPainter::TextAntialiasing, true);

    p.fillRect(rect(), QColor(14, 16, 18));

    const QFont labelFont = font();
    const QFontMetrics fm(labelFont);

    const QRect area = rect().adjusted(4, 5, -4, -20);
    const int scaleW = fm.horizontalAdvance(QStringLiteral("-48")) + 6;
    const int gap = 5;
    const int barW = std::max(12, (area.width() - scaleW - gap * 2) / 2);
    const QRect scaleRect(area.left(), area.top(), scaleW, area.height());
    const QRect barL(scaleRect.right() + gap, area.top(), barW, area.height());
    const QRect barR(barL.right() + gap, area.top(), barW, area.height());

    auto dbToY = [&](float db, const QRect &bar) {
        const float t = (clampDb(db) - kMeterMinDb) / (kMeterMaxDb - kMeterMinDb);
        return bar.bottom() - static_cast<int>(std::llround(std::clamp(t, 0.f, 1.f) * bar.height()));
    };

    p.setFont(labelFont);
    p.setPen(QColor(175, 185, 198));
    for (float db : {0.f, -6.f, -12.f, -24.f, -36.f, -48.f}) {
        const int y = dbToY(db, barL);
        p.drawLine(scaleRect.right() - 3, y, scaleRect.right(), y);
        if (db == 0.f || db == -12.f || db == -24.f || db == -48.f) {
            const QString txt = (db > -0.05f) ? QStringLiteral("0")
                                              : QString::number(static_cast<int>(db));
            const QRect tr(scaleRect.left(), y - fm.height() / 2, scaleW - 4, fm.height());
            p.drawText(tr, Qt::AlignRight | Qt::AlignVCenter, txt);
        }
    }

    auto drawChannel = [&](const QRect &bar, float levelDb, float capDb, const char *chLabel) {
        QRect well = bar.adjusted(0, 0, 0, 0);
        p.fillRect(well, QColor(8, 9, 11));
        p.setPen(QColor(38, 44, 52));
        p.drawRect(well);

        const int segStep = kSegPx + kSegGap;
        const int innerTop = well.top() + 2;
        const int innerBottom = well.bottom() - 2;
        const int innerH = std::max(1, innerBottom - innerTop);
        const int segCount = innerH / segStep;

        const int yLevel = dbToY(levelDb, well);
        const int yCap = dbToY(capDb, well);

        for (int s = 0; s < segCount; ++s) {
            const int segBottom = innerBottom - s * segStep;
            const int segTop = segBottom - kSegPx;
            if (segTop < innerTop) {
                break;
            }
            const float rel = static_cast<float>(innerBottom - segBottom) / static_cast<float>(innerH);
            const float segDb = kMeterMinDb + rel * (kMeterMaxDb - kMeterMinDb);

            if (segTop >= yLevel - 1 && segTop >= innerTop) {
                QRect seg(well.left() + 2, segTop, well.width() - 4, kSegPx);
                p.fillRect(seg, segmentColor(segDb + static_cast<float>(kSegPx) * 0.5f));
            }
        }

        if (capDb > kMeterMinDb + 0.5f) {
            const int capY = yCap - 2;
            QColor capCol = QColor(250, 248, 235);
            if (capDb >= -3.f) {
                capCol = QColor(255, 90, 75);
            } else if (capDb >= -9.f) {
                capCol = QColor(255, 210, 70);
            }
            QRect cap(well.left() - 1, capY, well.width() + 2, 5);
            p.setPen(QPen(capCol.darker(115), 1));
            p.setBrush(capCol);
            p.drawRoundedRect(cap, 2, 2);
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(255, 255, 255, 55));
            p.drawRoundedRect(cap.adjusted(1, 0, -1, -2), 1, 1);
        }

        p.setPen(QColor(200, 210, 222));
        p.setFont(labelFont);
        const QRect lr(bar.left(), rect().bottom() - 17, bar.width(), 16);
        p.drawText(lr, Qt::AlignHCenter | Qt::AlignVCenter, QString::fromLatin1(chLabel));
    };

    drawChannel(barL, levelL, capHoldL, "L");
    drawChannel(barR, levelR, capHoldR, "R");

    p.setPen(QColor(42, 48, 56));
    p.drawRect(rect().adjusted(0, 0, -1, -1));
}
