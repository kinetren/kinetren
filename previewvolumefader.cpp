#include "previewvolumefader.h"

#include <QFontDatabase>
#include <QFontMetrics>
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
    setFixedWidth(44);
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
    return rect().adjusted(10, 8, -10, -26);
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
    const QRect muteRect((width() - 26) / 2, height() - 20, 26, 16);
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
    return QSize(44, 220);
}

QSize PreviewVolumeFader::minimumSizeHint() const
{
    return QSize(40, 120);
}

void PreviewVolumeFader::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setRenderHint(QPainter::TextAntialiasing, true);
    p.fillRect(rect(), QColor(18, 20, 23));

    const QFont textFont = font();
    const QFontMetrics fm(textFont);
    p.setFont(textFont);

    const QRect rail = railRect();
    p.fillRect(rail, QColor(26, 30, 34));
    p.setPen(QColor(50, 58, 68));
    p.drawRect(rail);

    p.setPen(QColor(90, 100, 115));
    for (float db : {12.f, 0.f, -12.f, -24.f}) {
        const int y = yForDb(db);
        p.drawLine(rail.left(), y, rail.right(), y);
    }

    const int handleY = muted_ ? rail.bottom() : yForDb(volumeDb_);
    p.setPen(QPen(QColor(35, 40, 48), 1));
    p.setBrush(muted_ ? QColor(160, 55, 55) : QColor(72, 148, 210));
    p.drawRoundedRect(rail.left() + 1, handleY - 5, rail.width() - 2, 10, 2, 2);

    if (dragging_ || muted_) {
        const QString t =
            muted_ ? QStringLiteral("MUTE") : QStringLiteral("%1 dB").arg(volumeDb_, 0, 'f', volumeDb_ >= 10.f ? 0 : 1);
        const int tw = fm.horizontalAdvance(t) + 12;
        QRect box((width() - tw) / 2, rail.top() + 6, tw, fm.height() + 8);
        p.fillRect(box, QColor(8, 10, 12, 245));
        p.setPen(QColor(55, 65, 78));
        p.drawRect(box);
        p.setPen(QColor(245, 248, 252));
        p.drawText(box, Qt::AlignCenter, t);
    }

    const QRect muteRect((width() - 26) / 2, height() - 20, 26, 16);
    p.setPen(QColor(190, 200, 215));
    p.drawRoundedRect(muteRect, 3, 3);
    p.drawText(muteRect, Qt::AlignCenter, QStringLiteral("M"));
}
