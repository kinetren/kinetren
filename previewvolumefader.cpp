#include "previewvolumefader.h"

#include <QPainter>
#include <QMouseEvent>

#include <algorithm>
#include <cmath>

namespace {
constexpr float kMinDb = -60.f;
constexpr float kMaxDb = 24.f;
constexpr float kDefaultDb = 0.f;
} // namespace

PreviewVolumeFader::PreviewVolumeFader(QWidget *parent)
    : QWidget(parent)
{
    setFixedWidth(40);
    setCursor(Qt::SizeVerCursor);
    setToolTip(tr("Master preview volume. Double-click: 0 dB. M: mute."));
}

void PreviewVolumeFader::setVolumeDb(float db)
{
    db = std::clamp(db, kMinDb, kMaxDb);
    if (std::abs(db - volumeDb_) < 1e-4f && !muted_) {
        return;
    }
    volumeDb_ = db;
    muted_ = false;
    update();
    emitVolume();
}

void PreviewVolumeFader::setMuted(bool muted)
{
    if (muted_ == muted) {
        return;
    }
    muted_ = muted;
    update();
    emitVolume();
}

float PreviewVolumeFader::dbToLinear(float db)
{
    if (db <= kMinDb + 0.5f) {
        return 0.f;
    }
    return std::pow(10.f, db / 20.f);
}

float PreviewVolumeFader::linearToDb(float linear)
{
    if (linear <= 1e-6f) {
        return kMinDb;
    }
    return 20.f * std::log10(linear);
}

int PreviewVolumeFader::yForDb(float db) const
{
    const QRect r = railRect();
    const float t = (db - kMinDb) / (kMaxDb - kMinDb);
    return r.bottom() - static_cast<int>(std::llround(std::clamp(t, 0.f, 1.f) * r.height()));
}

float PreviewVolumeFader::dbForY(int y) const
{
    const QRect r = railRect();
    if (r.height() <= 0) {
        return kDefaultDb;
    }
    const float t = static_cast<float>(r.bottom() - y) / static_cast<float>(r.height());
    return kMinDb + std::clamp(t, 0.f, 1.f) * (kMaxDb - kMinDb);
}

QRect PreviewVolumeFader::railRect() const
{
    return rect().adjusted(8, 6, -8, -22);
}

void PreviewVolumeFader::applyFromY(int y)
{
    muted_ = false;
    volumeDb_ = dbForY(y);
    update();
    emitVolume();
}

void PreviewVolumeFader::emitVolume()
{
    const float lin = muted_ ? 0.f : dbToLinear(volumeDb_);
    emit volumeChanged(lin);
    emit volumeDbChanged(muted_ ? kMinDb : volumeDb_);
}

void PreviewVolumeFader::mousePressEvent(QMouseEvent *event)
{
    if (event->button() != Qt::LeftButton) {
        return;
    }
    const QRect muteRect((width() - 22) / 2, height() - 18, 22, 14);
    if (muteRect.contains(event->pos())) {
        setMuted(!muted_);
        return;
    }
    dragging_ = true;
    applyFromY(event->pos().y());
}

void PreviewVolumeFader::mouseMoveEvent(QMouseEvent *event)
{
    if (dragging_) {
        applyFromY(event->pos().y());
    }
}

void PreviewVolumeFader::mouseReleaseEvent(QMouseEvent *event)
{
    Q_UNUSED(event);
    dragging_ = false;
}

void PreviewVolumeFader::mouseDoubleClickEvent(QMouseEvent *event)
{
    Q_UNUSED(event);
    setVolumeDb(kDefaultDb);
}

QSize PreviewVolumeFader::sizeHint() const
{
    return QSize(40, 220);
}

QSize PreviewVolumeFader::minimumSizeHint() const
{
    return QSize(36, 120);
}

void PreviewVolumeFader::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.fillRect(rect(), QColor(22, 24, 27));

    const QRect rail = railRect();
    p.fillRect(rail, QColor(30, 34, 38));
    p.setPen(QColor(55, 62, 70));
    p.drawRect(rail);

    p.setPen(QColor(75, 85, 98));
    p.setFont(QFont(QStringLiteral("Sans"), 7));
    for (float db : {12.f, 0.f, -12.f, -24.f, -48.f}) {
        const int y = yForDb(db);
        p.drawLine(rail.left(), y, rail.right(), y);
        if (db == 0.f) {
            p.setPen(QColor(140, 155, 175));
            p.drawText(QRect(rail.left() - 1, y - 7, rail.width() + 2, 14), Qt::AlignHCenter, QStringLiteral("0"));
            p.setPen(QColor(75, 85, 98));
        }
    }

    const int handleY = muted_ ? rail.bottom() : yForDb(volumeDb_);
    p.setPen(Qt::NoPen);
    p.setBrush(muted_ ? QColor(150, 65, 65) : QColor(95, 165, 220));
    p.drawRoundedRect(rail.left() - 1, handleY - 4, rail.width() + 2, 8, 3, 3);

    if (dragging_ || muted_) {
        QRect valueBox((width() - 36) / 2, rail.top() + 4, 36, 16);
        p.fillRect(valueBox, QColor(12, 14, 16, 230));
        p.setPen(QColor(230, 236, 245));
        p.setFont(QFont(QStringLiteral("Sans"), 8, QFont::Bold));
        const QString t = muted_ ? QStringLiteral("M")
                                 : QStringLiteral("%1").arg(volumeDb_, 0, 'f', volumeDb_ >= 10.f ? 0 : 1);
        p.drawText(valueBox, Qt::AlignCenter, t);
    }

    const QRect muteRect((width() - 22) / 2, height() - 18, 22, 14);
    p.setPen(QColor(175, 185, 198));
    p.drawRoundedRect(muteRect, 2, 2);
    p.drawText(muteRect, Qt::AlignCenter, QStringLiteral("M"));
}
