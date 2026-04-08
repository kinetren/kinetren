#include "timelineeditor.h"

#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFileInfo>
#include <QMimeData>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QUrl>
#include <QtGlobal>

#include <algorithm>
#include <cstdlib>

namespace {
constexpr qint64 kSnapThresholdMs = 250;

QPoint eventPos(const QMouseEvent *event)
{
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    return event->position().toPoint();
#else
    return event->pos();
#endif
}

QPoint eventPos(const QDropEvent *event)
{
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    return event->position().toPoint();
#else
    return event->pos();
#endif
}
}

TimelineEditor::TimelineEditor(QWidget *parent)
    : QWidget(parent)
{
    setAcceptDrops(true);
    setMouseTracking(true);
    setMinimumHeight(210);
}

qint64 TimelineEditor::cursorMs() const
{
    return cursorPositionMs;
}

void TimelineEditor::setCursorMs(qint64 valueMs)
{
    const qint64 bounded = std::max<qint64>(0, valueMs);
    if (bounded == cursorPositionMs) {
        return;
    }

    cursorPositionMs = bounded;
    emit cursorChanged(cursorPositionMs);
    emitOverlayInfo();
    update();
}

void TimelineEditor::addMarkerAtCursor()
{
    markersMs.push_back(cursorPositionMs);
    std::sort(markersMs.begin(), markersMs.end());
    update();
}

std::optional<TimelineEditor::ActiveClip> TimelineEditor::activeVideoAt(qint64 timeMs) const
{
    for (auto it = clips.rbegin(); it != clips.rend(); ++it) {
        if (it->track != TrackType::Video) {
            continue;
        }
        const bool active = timeMs >= it->startMs && timeMs < (it->startMs + it->durationMs);
        if (!active) {
            continue;
        }
        return ActiveClip{it->sourcePath, it->startMs, it->durationMs};
    }
    return std::nullopt;
}

std::vector<TimelineEditor::ActiveClip> TimelineEditor::activeAudioAt(qint64 timeMs) const
{
    std::vector<ActiveClip> result;
    for (const auto &clip : clips) {
        if (clip.track != TrackType::Audio) {
            continue;
        }
        const bool active = timeMs >= clip.startMs && timeMs < (clip.startMs + clip.durationMs);
        if (!active) {
            continue;
        }
        result.push_back(ActiveClip{clip.sourcePath, clip.startMs, clip.durationMs});
    }
    return result;
}

void TimelineEditor::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);

    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.fillRect(rect(), QColor(18, 18, 18));

    const QRect rulerRect(timelineLeft(), rulerTop(), timelineRight() - timelineLeft(), rulerHeight());
    const QRect videoRect(timelineLeft(), videoTrackTop(), timelineRight() - timelineLeft(), trackHeight());
    const QRect audioRect(timelineLeft(), audioTrackTop(), timelineRight() - timelineLeft(), trackHeight());

    p.fillRect(rulerRect, QColor(30, 30, 30));
    p.fillRect(videoRect, QColor(36, 46, 58));
    p.fillRect(audioRect, QColor(48, 38, 38));

    p.setPen(QColor(100, 100, 100));
    p.drawText(8, videoTrackTop() + 22, "Video");
    p.drawText(8, audioTrackTop() + 22, "Audio");

    const qint64 visibleDurationMs =
        static_cast<qint64>((timelineRight() - timelineLeft()) / pixelsPerSecond() * 1000.0);
    const qint64 maxTimeMs = std::max<qint64>(visibleDurationMs, cursorPositionMs + 1000);
    for (qint64 t = 0; t <= maxTimeMs; t += 1000) {
        const int x = timeToX(t);
        p.setPen(QColor(70, 70, 70));
        p.drawLine(x, rulerTop(), x, audioTrackTop() + trackHeight());
        p.setPen(QColor(170, 170, 170));
        p.drawText(x + 2, rulerTop() + 14, QString::number(t / 1000.0, 'f', 1));
    }

    for (qint64 marker : markersMs) {
        const int x = timeToX(marker);
        p.setPen(QColor(255, 195, 0));
        p.drawLine(x, rulerTop(), x, audioTrackTop() + trackHeight());
        QPainterPath triangle;
        triangle.moveTo(x, rulerTop());
        triangle.lineTo(x - 5, rulerTop() + 8);
        triangle.lineTo(x + 5, rulerTop() + 8);
        triangle.closeSubpath();
        p.fillPath(triangle, QColor(255, 195, 0));
    }

    for (const auto &clip : clips) {
        const int x = timeToX(clip.startMs);
        const int w = std::max(12, timeToX(clip.startMs + clip.durationMs) - x);
        const int y = (clip.track == TrackType::Video) ? videoTrackTop() + 5 : audioTrackTop() + 5;
        const QRect clipRect(x, y, w, trackHeight() - 10);

        p.setPen((clip.id == selectedClipId) ? QColor(255, 255, 255) : QColor(25, 25, 25));
        p.setBrush(clip.color);
        p.drawRoundedRect(clipRect, 4, 4);

        p.setPen(QColor(230, 230, 230));
        p.drawText(clipRect.adjusted(6, 0, -6, 0), Qt::AlignVCenter | Qt::AlignLeft, QFileInfo(clip.sourcePath).fileName());
    }

    const int cursorX = timeToX(cursorPositionMs);
    p.setPen(QPen(QColor(255, 80, 80), 2));
    p.drawLine(cursorX, rulerTop(), cursorX, audioTrackTop() + trackHeight());
}

void TimelineEditor::mousePressEvent(QMouseEvent *event)
{
    const QPoint pos = eventPos(event);
    const int x = pos.x();
    const int y = pos.y();

    selectedClipId = -1;
    for (const auto &clip : clips) {
        const int clipX = timeToX(clip.startMs);
        const int clipW = std::max(12, timeToX(clip.startMs + clip.durationMs) - clipX);
        const int clipY = (clip.track == TrackType::Video) ? videoTrackTop() + 5 : audioTrackTop() + 5;
        const QRect clipRect(clipX, clipY, clipW, trackHeight() - 10);
        if (clipRect.contains(x, y)) {
            selectedClipId = clip.id;
            break;
        }
    }

    setCursorMs(xToTimeMs(x));
    update();
}

void TimelineEditor::dragEnterEvent(QDragEnterEvent *event)
{
    if (event->mimeData()->hasUrls() || event->mimeData()->hasText()) {
        event->acceptProposedAction();
        return;
    }
    QWidget::dragEnterEvent(event);
}

void TimelineEditor::dragMoveEvent(QDragMoveEvent *event)
{
    if (event->mimeData()->hasUrls() || event->mimeData()->hasText()) {
        event->acceptProposedAction();
        return;
    }
    QWidget::dragMoveEvent(event);
}

void TimelineEditor::dropEvent(QDropEvent *event)
{
    const int x = eventPos(event).x();
    const qint64 dropStartMs = snapTimeMs(xToTimeMs(x));

    if (event->mimeData()->hasUrls()) {
        for (const auto &url : event->mimeData()->urls()) {
            if (url.isLocalFile()) {
                addClipFromDrop(url.toLocalFile(), dropStartMs);
            }
        }
        event->acceptProposedAction();
        return;
    }

    if (event->mimeData()->hasText()) {
        addClipFromDrop(event->mimeData()->text().trimmed(), dropStartMs);
        event->acceptProposedAction();
        return;
    }

    QWidget::dropEvent(event);
}

int TimelineEditor::timelineLeft() const
{
    return 62;
}

int TimelineEditor::timelineRight() const
{
    return width() - 12;
}

int TimelineEditor::rulerTop() const
{
    return 10;
}

int TimelineEditor::rulerHeight() const
{
    return 22;
}

int TimelineEditor::videoTrackTop() const
{
    return rulerTop() + rulerHeight() + 8;
}

int TimelineEditor::audioTrackTop() const
{
    return videoTrackTop() + trackHeight() + 8;
}

int TimelineEditor::trackHeight() const
{
    return 64;
}

int TimelineEditor::bottomPadding() const
{
    return 8;
}

double TimelineEditor::pixelsPerSecond() const
{
    return 90.0;
}

qint64 TimelineEditor::xToTimeMs(int x) const
{
    const int relative = std::clamp(x - timelineLeft(), 0, std::max(0, timelineRight() - timelineLeft()));
    return static_cast<qint64>(relative / pixelsPerSecond() * 1000.0);
}

int TimelineEditor::timeToX(qint64 timeMs) const
{
    const int px = timelineLeft() + static_cast<int>((timeMs / 1000.0) * pixelsPerSecond());
    return std::clamp(px, timelineLeft(), timelineRight());
}

int TimelineEditor::trackAtY(int y) const
{
    if (y >= videoTrackTop() && y < videoTrackTop() + trackHeight()) {
        return 0;
    }
    if (y >= audioTrackTop() && y < audioTrackTop() + trackHeight()) {
        return 1;
    }
    return -1;
}

qint64 TimelineEditor::snapTimeMs(qint64 candidateMs) const
{
    qint64 best = candidateMs;
    qint64 bestDistance = kSnapThresholdMs + 1;
    const auto consider = [&](qint64 v) {
        const qint64 d = std::llabs(v - candidateMs);
        if (d <= kSnapThresholdMs && d < bestDistance) {
            bestDistance = d;
            best = v;
        }
    };

    for (const auto &clip : clips) {
        consider(clip.startMs);
        consider(clip.startMs + clip.durationMs);
    }
    for (qint64 marker : markersMs) {
        consider(marker);
    }

    return std::max<qint64>(0, best);
}

qint64 TimelineEditor::defaultDurationForPath(const QString &path) const
{
    return isImagePath(path) ? 3000 : 5000;
}

bool TimelineEditor::isImagePath(const QString &path) const
{
    const QString ext = QFileInfo(path).suffix().toLower();
    return ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "bmp" || ext == "webp";
}

bool TimelineEditor::hasAudioForPath(const QString &path) const
{
    const QString ext = QFileInfo(path).suffix().toLower();
    if (isImagePath(path)) {
        return false;
    }
    if (ext == "mp3" || ext == "wav" || ext == "flac" || ext == "aac" || ext == "ogg") {
        return true;
    }
    return ext == "mp4" || ext == "mov" || ext == "mkv" || ext == "avi" || ext == "webm";
}

bool TimelineEditor::hasVideoForPath(const QString &path) const
{
    const QString ext = QFileInfo(path).suffix().toLower();
    if (isImagePath(path)) {
        return true;
    }
    return ext == "mp4" || ext == "mov" || ext == "mkv" || ext == "avi" || ext == "webm";
}

QColor TimelineEditor::colorFromPath(const QString &path) const
{
    const uint hash = qHash(path);
    const int r = 80 + static_cast<int>(hash % 120);
    const int g = 80 + static_cast<int>((hash / 7) % 120);
    const int b = 80 + static_cast<int>((hash / 17) % 120);
    return QColor(r, g, b);
}

void TimelineEditor::addClipFromDrop(const QString &path, qint64 startMs)
{
    if (path.isEmpty()) {
        return;
    }

    const qint64 durationMs = defaultDurationForPath(path);
    const bool audio = hasAudioForPath(path);
    const bool video = hasVideoForPath(path);
    const int linkedGroup = (video && audio) ? nextLinkedGroupId++ : -1;

    if (video) {
        ClipEvent v;
        v.id = nextClipId++;
        v.linkedGroupId = linkedGroup;
        v.track = TrackType::Video;
        v.startMs = startMs;
        v.durationMs = durationMs;
        v.sourcePath = path;
        v.color = colorFromPath(path + ":v");
        clips.push_back(v);
    }

    if (audio) {
        ClipEvent a;
        a.id = nextClipId++;
        a.linkedGroupId = linkedGroup;
        a.track = TrackType::Audio;
        a.startMs = startMs;
        a.durationMs = durationMs;
        a.sourcePath = path;
        a.color = colorFromPath(path + ":a");
        clips.push_back(a);
    }

    setCursorMs(startMs);
    emitOverlayInfo();
    update();
}

void TimelineEditor::emitOverlayInfo()
{
    QString activeVideoName = "-";
    int activeAudioCount = 0;

    for (const auto &clip : clips) {
        const bool active = cursorPositionMs >= clip.startMs &&
                            cursorPositionMs < (clip.startMs + clip.durationMs);
        if (!active) {
            continue;
        }
        if (clip.track == TrackType::Video) {
            activeVideoName = QFileInfo(clip.sourcePath).fileName();
        } else {
            ++activeAudioCount;
        }
    }

    const QString text = QString("Overlay @ %1s | Video: %2 | Audio layers: %3")
                             .arg(cursorPositionMs / 1000.0, 0, 'f', 2)
                             .arg(activeVideoName)
                             .arg(activeAudioCount);
    if (text != lastOverlayInfo) {
        lastOverlayInfo = text;
        emit overlayInfoChanged(text);
    }
}
