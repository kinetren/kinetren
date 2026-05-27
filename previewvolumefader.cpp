#include "previewvolumefader.h"

#include <QFontDatabase>
#include <QLinearGradient>
#include <QMouseEvent>
#include <QPainter>

#include <algorithm>
#include <cmath>

namespace {
constexpr float kMinDb = -60.f;
constexpr float kMaxDb = 24.f;
constexpr float kDefaultDb = 0.f;
constexpr int kMuteBtnW = 28;
constexpr int kMuteBtnH = 34;
constexpr int kMuteBandH = 40;
constexpr int kHandleMarginH = 3;
constexpr int kHandleH = 14;
constexpr int kCapRadius = 3;
} // namespace

PreviewVolumeFader::PreviewVolumeFader(QWidget *parent)
    : QWidget(parent)
{
    setAttribute(Qt::WA_TranslucentBackground, true);
    setAutoFillBackground(false);
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    setCursor(Qt::SizeVerCursor);
    setToolTip(tr("Master preview volume. Double-click: 0 dB. M: mute."));
    QFont f = QFontDatabase::systemFont(QFontDatabase::GeneralFont);
    f.setPointSize(9);
    setFont(f);
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
    setActivity(muted_ || dragging_);
    update();
    emitVolume();
}

void PreviewVolumeFader::setActivity(bool active)
{
    emit activityChanged(active);
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

QRect PreviewVolumeFader::railRect() const
{
    return QRect(kHandleMarginH, 4, width() - 2 * kHandleMarginH, height() - kMuteBandH - 6);
}

QRect PreviewVolumeFader::handleRectForY(int centerY) const
{
    const QRect rail = railRect();
    const int top = std::clamp(centerY - kHandleH / 2, rail.top(), rail.bottom() - kHandleH);
    return QRect(rail.left(), top, rail.width(), kHandleH);
}

QRect PreviewVolumeFader::muteButtonRect() const
{
    return QRect((width() - kMuteBtnW) / 2, height() - kMuteBtnH - 5, kMuteBtnW, kMuteBtnH);
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
    if (muteButtonRect().contains(event->pos())) {
        setMuted(!muted_);
        return;
    }
    dragging_ = true;
    setActivity(true);
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
    if (dragging_) {
        dragging_ = false;
        setActivity(muted_);
    }
}

void PreviewVolumeFader::mouseDoubleClickEvent(QMouseEvent *event)
{
    Q_UNUSED(event);
    setVolumeDb(kDefaultDb);
}

QSize PreviewVolumeFader::sizeHint() const
{
    return QSize(48, 220);
}

QSize PreviewVolumeFader::minimumSizeHint() const
{
    return QSize(36, 120);
}

void PreviewVolumeFader::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);

    const QRect rail = railRect();
    const int handleY = muted_ ? rail.bottom() : yForDb(volumeDb_);
    const QRect cap = handleRectForY(handleY);
    p.setPen(Qt::NoPen);

    const QColor base = muted_ ? QColor(172, 52, 52) : QColor(78, 82, 90);
    const QColor topHi = muted_ ? QColor(210, 90, 90) : QColor(128, 132, 142);
    const QColor botSh = muted_ ? QColor(110, 32, 32) : QColor(48, 50, 56);
    QLinearGradient capGrad(cap.topLeft(), cap.bottomLeft());
    capGrad.setColorAt(0.0, topHi);
    capGrad.setColorAt(0.45, base);
    capGrad.setColorAt(1.0, botSh);
    p.setPen(QPen(QColor(12, 14, 18), 1));
    p.setBrush(capGrad);
    p.drawRoundedRect(cap, kCapRadius, kCapRadius);
    p.setPen(QPen(QColor(0, 0, 0, 70), 1));
    p.drawLine(cap.left() + 5, cap.bottom() - 1, cap.right() - 5, cap.bottom() - 1);
    p.setPen(QPen(QColor(255, 255, 255, 40), 1));
    for (int gx = cap.left() + 7; gx < cap.right() - 7; gx += 4) {
        p.drawLine(gx, cap.top() + 3, gx, cap.bottom() - 3);
    }
    p.fillRect(QRect(cap.center().x() - 1, cap.top() + 3, 2, cap.height() - 6), QColor(0, 0, 0, 45));

    const QRect muteRect = muteButtonRect();
    p.fillRect(muteRect, QColor(32, 36, 42, 190));
    p.setPen(QColor(220, 228, 240));
    QFont muteFont = font();
    muteFont.setPointSize(11);
    muteFont.setBold(true);
    p.setFont(muteFont);
    p.drawText(muteRect, Qt::AlignCenter, QStringLiteral("M"));
}
