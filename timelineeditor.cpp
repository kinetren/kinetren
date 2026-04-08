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
#include <system_error>

#include <avcpp/avutils.h>
#include <avcpp/formatcontext.h>

namespace {
constexpr qint64 kSnapThresholdMs = 250;
constexpr qint64 kMinClipDurationMs = 80;
constexpr int kTrimHandlePx = 7;

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
    tracks.push_back({TrackType::Video, "V1"});
    tracks.push_back({TrackType::Audio, "A1"});
    setMinimumHeight(260);
    refreshTimelineRangeSignals();
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
    ensureCursorVisible();
    emit cursorChanged(cursorPositionMs);
    emitOverlayInfo();
    refreshTimelineRangeSignals();
    update();
}

void TimelineEditor::addMarkerAtCursor()
{
    markersMs.push_back(cursorPositionMs);
    std::sort(markersMs.begin(), markersMs.end());
    refreshTimelineRangeSignals();
    update();
}

void TimelineEditor::addVideoTrack()
{
    int count = 0;
    for (const auto &track : tracks) {
        if (track.type == TrackType::Video) {
            ++count;
        }
    }
    tracks.push_back({TrackType::Video, QString("V%1").arg(count + 1)});
    setMinimumHeight(contentBottom() + bottomPadding());
    refreshTimelineRangeSignals();
    update();
}

void TimelineEditor::addAudioTrack()
{
    int count = 0;
    for (const auto &track : tracks) {
        if (track.type == TrackType::Audio) {
            ++count;
        }
    }
    tracks.push_back({TrackType::Audio, QString("A%1").arg(count + 1)});
    setMinimumHeight(contentBottom() + bottomPadding());
    refreshTimelineRangeSignals();
    update();
}

bool TimelineEditor::splitSelectedClipAtCursor()
{
    ClipEvent *selected = findClipById(selectedClipId);
    if (!selected) {
        return false;
    }

    const int linkedGroupId = selected->linkedGroupId;
    std::vector<int> candidates;
    for (const auto &clip : clips) {
        const bool target = (linkedGroupId >= 0) ? (clip.linkedGroupId == linkedGroupId) : (clip.id == selectedClipId);
        if (!target) {
            continue;
        }
        if (cursorPositionMs <= clip.startMs || cursorPositionMs >= (clip.startMs + clip.durationMs)) {
            continue;
        }
        candidates.push_back(clip.id);
    }
    if (candidates.empty()) {
        return false;
    }

    std::vector<ClipEvent> additions;
    int newSelectedId = selectedClipId;
    for (int id : candidates) {
        ClipEvent *clip = findClipById(id);
        if (!clip) {
            continue;
        }
        const qint64 splitOffset = cursorPositionMs - clip->startMs;
        if (splitOffset <= kMinClipDurationMs || (clip->durationMs - splitOffset) <= kMinClipDurationMs) {
            continue;
        }

        ClipEvent right = *clip;
        right.id = nextClipId++;
        right.startMs = cursorPositionMs;
        right.durationMs = clip->durationMs - splitOffset;
        right.sourceInMs = clip->sourceInMs + splitOffset;
        clip->durationMs = splitOffset;
        additions.push_back(right);
        if (id == selectedClipId) {
            newSelectedId = right.id;
        }
    }

    if (additions.empty()) {
        return false;
    }
    clips.insert(clips.end(), additions.begin(), additions.end());
    selectedClipId = newSelectedId;
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    update();
    return true;
}

bool TimelineEditor::trimSelectedClipStartToCursor()
{
    ClipEvent *clip = findClipById(selectedClipId);
    if (!clip) {
        return false;
    }
    qint64 newStart = std::clamp(cursorPositionMs, clip->startMs, clip->startMs + clip->durationMs - kMinClipDurationMs);
    const qint64 maxSourceIn = std::max<qint64>(0, clip->sourceDurationMs - kMinClipDurationMs);
    const qint64 delta = newStart - clip->startMs;
    const qint64 newSourceIn = std::clamp(clip->sourceInMs + delta, 0LL, maxSourceIn);
    newStart = clip->startMs + (newSourceIn - clip->sourceInMs);
    if (newStart <= clip->startMs) {
        return false;
    }
    clip->durationMs -= (newStart - clip->startMs);
    clip->startMs = newStart;
    clip->sourceInMs = newSourceIn;
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    update();
    return true;
}

bool TimelineEditor::trimSelectedClipEndToCursor()
{
    ClipEvent *clip = findClipById(selectedClipId);
    if (!clip) {
        return false;
    }
    const qint64 absoluteStart = clip->startMs;
    const qint64 absoluteEnd = clip->startMs + clip->durationMs;
    const qint64 maxEnd = clip->startMs + std::max<qint64>(kMinClipDurationMs, clip->sourceDurationMs - clip->sourceInMs);
    const qint64 newEnd = std::clamp(cursorPositionMs, absoluteStart + kMinClipDurationMs, std::max(absoluteEnd, maxEnd));
    if (newEnd >= absoluteEnd) {
        return false;
    }
    clip->durationMs = newEnd - clip->startMs;
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    update();
    return true;
}

bool TimelineEditor::moveSelectedClipToAdjacentTrack(int direction)
{
    if (direction == 0) {
        return false;
    }
    ClipEvent *clip = findClipById(selectedClipId);
    if (!clip || !isTrackIndexValid(clip->trackIndex)) {
        return false;
    }
    const TrackType type = clip->mediaType;
    int target = clip->trackIndex + ((direction > 0) ? 1 : -1);
    while (isTrackIndexValid(target)) {
        if (tracks[static_cast<size_t>(target)].type == type) {
            clip->trackIndex = target;
            update();
            return true;
        }
        target += ((direction > 0) ? 1 : -1);
    }
    return false;
}

void TimelineEditor::setLoopEnabled(bool enabled)
{
    if (loopRegionEnabled == enabled) {
        return;
    }
    loopRegionEnabled = enabled;
    emit loopEnabledChanged(loopRegionEnabled);
    emitOverlayInfo();
    update();
}

bool TimelineEditor::loopEnabled() const
{
    return loopRegionEnabled;
}

bool TimelineEditor::hasSelectionRegion() const
{
    return hasRegionSelection;
}

qint64 TimelineEditor::selectionRegionStartMs() const
{
    return regionStartMs;
}

qint64 TimelineEditor::selectionRegionEndMs() const
{
    return regionEndMs;
}

void TimelineEditor::setSelectionRegionFromCursor(bool setStart)
{
    if (!hasRegionSelection) {
        regionStartMs = cursorPositionMs;
        regionEndMs = cursorPositionMs + 1000;
        hasRegionSelection = true;
    }
    if (setStart) {
        regionStartMs = std::min(cursorPositionMs, regionEndMs);
    } else {
        regionEndMs = std::max(cursorPositionMs, regionStartMs);
    }
    setRegionBounds(regionStartMs, regionEndMs);
}

void TimelineEditor::clearSelectionRegion()
{
    if (!hasRegionSelection) {
        return;
    }
    hasRegionSelection = false;
    regionStartMs = 0;
    regionEndMs = 0;
    emitRegionChanged();
    refreshTimelineRangeSignals();
    update();
}

qint64 TimelineEditor::maxTimelineMs() const
{
    return recomputeMaxTimelineMs();
}

qint64 TimelineEditor::visibleDurationMs() const
{
    return contentVisibleMs();
}

qint64 TimelineEditor::viewStartMs() const
{
    return timelineViewStartMs;
}

qint64 TimelineEditor::maxViewStartMs() const
{
    return std::max<qint64>(0, recomputeMaxTimelineMs() - contentVisibleMs());
}

void TimelineEditor::setViewStartMs(qint64 valueMs)
{
    const qint64 bounded = std::clamp<qint64>(valueMs, 0, maxViewStartMs());
    if (bounded == timelineViewStartMs) {
        return;
    }
    timelineViewStartMs = bounded;
    emit viewStartChanged(timelineViewStartMs, maxViewStartMs());
    update();
}

std::optional<TimelineEditor::ActiveClip> TimelineEditor::activeVideoAt(qint64 timeMs) const
{
    for (auto it = clips.rbegin(); it != clips.rend(); ++it) {
        if (!isTrackIndexValid(it->trackIndex) || it->mediaType != TrackType::Video) {
            continue;
        }
        const bool active = timeMs >= it->startMs && timeMs < (it->startMs + it->durationMs);
        if (!active) {
            continue;
        }
        return ActiveClip{it->sourcePath, it->startMs, it->durationMs, it->sourceInMs};
    }
    return std::nullopt;
}

std::vector<TimelineEditor::ActiveClip> TimelineEditor::activeVideoLayersAt(qint64 timeMs) const
{
    std::vector<ActiveClip> result;
    for (const auto &clip : clips) {
        if (!isTrackIndexValid(clip.trackIndex) || clip.mediaType != TrackType::Video) {
            continue;
        }
        const bool active = timeMs >= clip.startMs && timeMs < (clip.startMs + clip.durationMs);
        if (!active) {
            continue;
        }
        result.push_back(ActiveClip{clip.sourcePath, clip.startMs, clip.durationMs, clip.sourceInMs});
    }
    return result;
}

std::vector<TimelineEditor::ActiveClip> TimelineEditor::activeAudioAt(qint64 timeMs) const
{
    std::vector<ActiveClip> result;
    for (const auto &clip : clips) {
        if (!isTrackIndexValid(clip.trackIndex) || clip.mediaType != TrackType::Audio) {
            continue;
        }
        const bool active = timeMs >= clip.startMs && timeMs < (clip.startMs + clip.durationMs);
        if (!active) {
            continue;
        }
        result.push_back(ActiveClip{clip.sourcePath, clip.startMs, clip.durationMs, clip.sourceInMs});
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
    p.fillRect(rulerRect, QColor(30, 30, 30));
    p.fillRect(QRect(0, rulerTop(), headerWidth(), rulerHeight()), QColor(26, 26, 26));

    for (int i = 0; i < static_cast<int>(tracks.size()); ++i) {
        const QRect laneRect(timelineLeft(), trackTop(i), timelineRight() - timelineLeft(), trackHeight());
        const QColor laneColor = (tracks[static_cast<size_t>(i)].type == TrackType::Video) ? QColor(36, 46, 58) : QColor(48, 38, 38);
        p.fillRect(laneRect, laneColor);
        p.fillRect(QRect(0, trackTop(i), headerWidth(), trackHeight()), QColor(24, 24, 24));
        p.setPen(QColor(100, 100, 100));
        p.drawText(8, trackTop(i) + 24, tracks[static_cast<size_t>(i)].name);
    }

    const qint64 viewEndMs = timelineViewStartMs + contentVisibleMs();
    const qint64 tickStart = (timelineViewStartMs / 1000) * 1000;
    for (qint64 t = std::max<qint64>(0, tickStart - 1000); t <= viewEndMs + 1000; t += 1000) {
        const int x = timeToX(t);
        p.setPen(QColor(70, 70, 70));
        p.drawLine(x, rulerTop(), x, contentBottom());
        p.setPen(QColor(170, 170, 170));
        p.drawText(x + 2, rulerTop() + 14, QString::number(t / 1000.0, 'f', 1));
    }

    if (hasRegionSelection && regionEndMs > regionStartMs) {
        const int x1 = timeToX(regionStartMs);
        const int x2 = timeToX(regionEndMs);
        const QRect regionRect(std::min(x1, x2), rulerTop(), std::abs(x2 - x1), contentBottom() - rulerTop());
        p.fillRect(regionRect, QColor(84, 124, 255, 42));
        p.setPen(QColor(102, 152, 255));
        p.drawLine(x1, rulerTop(), x1, contentBottom());
        p.drawLine(x2, rulerTop(), x2, contentBottom());
    }

    for (qint64 marker : markersMs) {
        const int x = timeToX(marker);
        p.setPen(QColor(255, 195, 0));
        p.drawLine(x, rulerTop(), x, contentBottom());
        QPainterPath triangle;
        triangle.moveTo(x, rulerTop());
        triangle.lineTo(x - 5, rulerTop() + 8);
        triangle.lineTo(x + 5, rulerTop() + 8);
        triangle.closeSubpath();
        p.fillPath(triangle, QColor(255, 195, 0));
    }

    for (const auto &clip : clips) {
        const QRect r = clipRect(clip);
        if (!r.intersects(QRect(timelineLeft(), rulerTop(), timelineRight() - timelineLeft(), contentBottom() - rulerTop()))) {
            continue;
        }
        p.setPen((clip.id == selectedClipId) ? QColor(255, 255, 255) : QColor(25, 25, 25));
        p.setBrush(clip.color);
        p.drawRoundedRect(r, 4, 4);
        p.setPen(QColor(230, 230, 230));
        p.drawText(r.adjusted(6, 0, -6, 0), Qt::AlignVCenter | Qt::AlignLeft, QFileInfo(clip.sourcePath).fileName());
        if (clip.id == selectedClipId) {
            p.setPen(QColor(255, 255, 255));
            p.drawLine(r.left() + 2, r.top() + 2, r.left() + 2, r.bottom() - 2);
            p.drawLine(r.right() - 2, r.top() + 2, r.right() - 2, r.bottom() - 2);
        }
    }

    const int cursorX = timeToX(cursorPositionMs);
    p.setPen(QPen(QColor(255, 80, 80), 2));
    p.drawLine(cursorX, rulerTop(), cursorX, contentBottom());

    if (loopRegionEnabled && hasRegionSelection && regionEndMs > regionStartMs) {
        p.setPen(QColor(120, 220, 120));
        p.drawText(timelineRight() - 90, rulerTop() + 15, "LOOP");
    }
}

void TimelineEditor::mousePressEvent(QMouseEvent *event)
{
    if (event->button() != Qt::LeftButton) {
        QWidget::mousePressEvent(event);
        return;
    }
    const QPoint pos = eventPos(event);
    setFocus(Qt::MouseFocusReason);

    selectedClipId = clipIdAt(pos);
    const qint64 pointerMs = snapTimeMs(xToTimeMs(pos.x()));
    setCursorMs(pointerMs);

    dragMode = DragMode::None;
    dragClipId = selectedClipId;
    if (selectedClipId >= 0) {
        ClipEvent *clip = findClipById(selectedClipId);
        if (clip) {
            dragAnchorMs = pointerMs;
            dragOriginalStartMs = clip->startMs;
            dragOriginalTrackIndex = clip->trackIndex;
            trimOriginalDurationMs = clip->durationMs;
            trimOriginalSourceInMs = clip->sourceInMs;
            if (isNearClipStartEdge(*clip, pos.x())) {
                dragMode = DragMode::TrimStart;
            } else if (isNearClipEndEdge(*clip, pos.x())) {
                dragMode = DragMode::TrimEnd;
            } else {
                dragMode = DragMode::MoveClip;
            }
        }
    } else if (pos.y() >= rulerTop() && pos.y() <= contentBottom()) {
        dragMode = DragMode::RegionSelect;
        regionAnchorMs = pointerMs;
        setRegionBounds(pointerMs, pointerMs);
    }

    update();
}

void TimelineEditor::mouseMoveEvent(QMouseEvent *event)
{
    const QPoint pos = eventPos(event);
    if (!(event->buttons() & Qt::LeftButton) || dragMode == DragMode::None) {
        const int hoverClip = clipIdAt(pos);
        if (hoverClip >= 0) {
            const ClipEvent *clip = findClipById(hoverClip);
            if (clip && (isNearClipStartEdge(*clip, pos.x()) || isNearClipEndEdge(*clip, pos.x()))) {
                setCursor(Qt::SizeHorCursor);
            } else {
                setCursor(Qt::OpenHandCursor);
            }
        } else {
            unsetCursor();
        }
        QWidget::mouseMoveEvent(event);
        return;
    }

    const qint64 pointerMs = snapTimeMs(xToTimeMs(pos.x()));
    setCursorMs(pointerMs);
    ClipEvent *clip = findClipById(dragClipId);
    if (dragMode == DragMode::MoveClip && clip) {
        const qint64 deltaMs = pointerMs - dragAnchorMs;
        clip->startMs = std::max<qint64>(0, dragOriginalStartMs + deltaMs);
        const int targetTrack = trackIndexAtY(pos.y());
        if (targetTrack >= 0 && tracks[static_cast<size_t>(targetTrack)].type == clip->mediaType) {
            clip->trackIndex = targetTrack;
        }
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::TrimStart && clip) {
        const qint64 originalEnd = dragOriginalStartMs + trimOriginalDurationMs;
        const qint64 maxSourceIn = std::max<qint64>(0, clip->sourceDurationMs - kMinClipDurationMs);
        qint64 wantedStart = std::clamp(pointerMs, dragOriginalStartMs, originalEnd - kMinClipDurationMs);
        qint64 newSourceIn = std::clamp(trimOriginalSourceInMs + (wantedStart - dragOriginalStartMs), 0LL, maxSourceIn);
        wantedStart = dragOriginalStartMs + (newSourceIn - trimOriginalSourceInMs);
        clip->startMs = wantedStart;
        clip->sourceInMs = newSourceIn;
        clip->durationMs = originalEnd - wantedStart;
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::TrimEnd && clip) {
        const qint64 minEnd = dragOriginalStartMs + kMinClipDurationMs;
        const qint64 maxEnd = dragOriginalStartMs + std::max<qint64>(kMinClipDurationMs, clip->sourceDurationMs - clip->sourceInMs);
        const qint64 wantedEnd = std::clamp(pointerMs, minEnd, std::max(minEnd, maxEnd));
        clip->durationMs = wantedEnd - dragOriginalStartMs;
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::RegionSelect) {
        setRegionBounds(regionAnchorMs, pointerMs);
    }
    emitOverlayInfo();
    update();
}

void TimelineEditor::mouseReleaseEvent(QMouseEvent *event)
{
    dragMode = DragMode::None;
    dragClipId = -1;
    unsetCursor();
    QWidget::mouseReleaseEvent(event);
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
    const QPoint pos = eventPos(event);
    const qint64 dropStartMs = snapTimeMs(xToTimeMs(pos.x()));
    const int preferredTrack = trackIndexAtY(pos.y());

    if (event->mimeData()->hasUrls()) {
        for (const auto &url : event->mimeData()->urls()) {
            if (url.isLocalFile()) {
                addClipFromDrop(url.toLocalFile(), dropStartMs, preferredTrack);
            }
        }
        event->acceptProposedAction();
        return;
    }

    if (event->mimeData()->hasText()) {
        addClipFromDrop(event->mimeData()->text().trimmed(), dropStartMs, preferredTrack);
        event->acceptProposedAction();
        return;
    }

    QWidget::dropEvent(event);
}

int TimelineEditor::timelineLeft() const
{
    return headerWidth();
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

int TimelineEditor::trackHeight() const
{
    return 58;
}

int TimelineEditor::trackGap() const
{
    return 6;
}

int TimelineEditor::headerWidth() const
{
    return 62;
}

int TimelineEditor::trackTop(int trackIndex) const
{
    return rulerTop() + rulerHeight() + 8 + trackIndex * (trackHeight() + trackGap());
}

int TimelineEditor::contentBottom() const
{
    if (tracks.empty()) {
        return rulerTop() + rulerHeight();
    }
    return trackTop(static_cast<int>(tracks.size()) - 1) + trackHeight();
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
    return timelineViewStartMs + static_cast<qint64>(relative / pixelsPerSecond() * 1000.0);
}

int TimelineEditor::timeToX(qint64 timeMs) const
{
    return timelineLeft() + static_cast<int>(((timeMs - timelineViewStartMs) / 1000.0) * pixelsPerSecond());
}

int TimelineEditor::trackIndexAtY(int y) const
{
    for (int i = 0; i < static_cast<int>(tracks.size()); ++i) {
        const int top = trackTop(i);
        if (y >= top && y < top + trackHeight()) {
            return i;
        }
    }
    return -1;
}

int TimelineEditor::maxTrackOfType(TrackType type) const
{
    int result = -1;
    for (int i = 0; i < static_cast<int>(tracks.size()); ++i) {
        if (tracks[static_cast<size_t>(i)].type == type) {
            result = i;
        }
    }
    return result;
}

bool TimelineEditor::isTrackIndexValid(int trackIndex) const
{
    return trackIndex >= 0 && trackIndex < static_cast<int>(tracks.size());
}

QRect TimelineEditor::clipRect(const ClipEvent &clip) const
{
    const int x = timeToX(clip.startMs);
    const int w = std::max(12, timeToX(clip.startMs + clip.durationMs) - x);
    const int y = trackTop(clip.trackIndex) + 4;
    return QRect(x, y, w, trackHeight() - 8);
}

TimelineEditor::ClipEvent *TimelineEditor::findClipById(int id)
{
    for (auto &clip : clips) {
        if (clip.id == id) {
            return &clip;
        }
    }
    return nullptr;
}

const TimelineEditor::ClipEvent *TimelineEditor::findClipById(int id) const
{
    for (const auto &clip : clips) {
        if (clip.id == id) {
            return &clip;
        }
    }
    return nullptr;
}

int TimelineEditor::clipIdAt(const QPoint &pos) const
{
    for (auto it = clips.rbegin(); it != clips.rend(); ++it) {
        if (!isTrackIndexValid(it->trackIndex)) {
            continue;
        }
        if (clipRect(*it).contains(pos)) {
            return it->id;
        }
    }
    return -1;
}

bool TimelineEditor::isNearClipStartEdge(const ClipEvent &clip, int x) const
{
    const QRect r = clipRect(clip);
    return std::abs(x - r.left()) <= kTrimHandlePx;
}

bool TimelineEditor::isNearClipEndEdge(const ClipEvent &clip, int x) const
{
    const QRect r = clipRect(clip);
    return std::abs(x - r.right()) <= kTrimHandlePx;
}

qint64 TimelineEditor::contentVisibleMs() const
{
    const int contentPx = std::max(1, timelineRight() - timelineLeft());
    return std::max<qint64>(1000, static_cast<qint64>(contentPx / pixelsPerSecond() * 1000.0));
}

qint64 TimelineEditor::recomputeMaxTimelineMs() const
{
    qint64 maxMs = cursorPositionMs + contentVisibleMs();
    for (const auto &clip : clips) {
        maxMs = std::max(maxMs, clip.startMs + clip.durationMs + 1000);
    }
    for (qint64 marker : markersMs) {
        maxMs = std::max(maxMs, marker + 1000);
    }
    if (hasRegionSelection) {
        maxMs = std::max(maxMs, regionEndMs + 1000);
    }
    return std::max<qint64>(contentVisibleMs(), maxMs);
}

void TimelineEditor::setRegionBounds(qint64 startMs, qint64 endMs)
{
    hasRegionSelection = true;
    regionStartMs = std::max<qint64>(0, std::min(startMs, endMs));
    regionEndMs = std::max(regionStartMs, std::max<qint64>(0, std::max(startMs, endMs)));
    emitRegionChanged();
    refreshTimelineRangeSignals();
    update();
}

void TimelineEditor::emitRegionChanged()
{
    emit selectionRegionChanged(hasRegionSelection, regionStartMs, regionEndMs);
}

void TimelineEditor::ensureCursorVisible()
{
    const qint64 visible = contentVisibleMs();
    if (cursorPositionMs < timelineViewStartMs) {
        timelineViewStartMs = cursorPositionMs;
        emit viewStartChanged(timelineViewStartMs, maxViewStartMs());
        return;
    }
    if (cursorPositionMs > timelineViewStartMs + visible) {
        timelineViewStartMs = cursorPositionMs - visible / 2;
        timelineViewStartMs = std::clamp<qint64>(timelineViewStartMs, 0, maxViewStartMs());
        emit viewStartChanged(timelineViewStartMs, maxViewStartMs());
    }
}

void TimelineEditor::refreshTimelineRangeSignals()
{
    const qint64 visible = contentVisibleMs();
    const qint64 maxTimeline = recomputeMaxTimelineMs();
    if (timelineViewStartMs > (maxTimeline - visible)) {
        timelineViewStartMs = std::max<qint64>(0, maxTimeline - visible);
    }
    emit timelineRangeChanged(maxTimeline, visible);
    emit viewStartChanged(timelineViewStartMs, maxViewStartMs());
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
    if (hasRegionSelection) {
        consider(regionStartMs);
        consider(regionEndMs);
    }

    return std::max<qint64>(0, best);
}

qint64 TimelineEditor::defaultDurationForPath(const QString &path) const
{
    if (isImagePath(path)) {
        return 3000;
    }

    using namespace av;
    std::error_code ec;
    FormatContext inputContext;
    inputContext.openInput(path.toStdString(), ec);
    if (ec) {
        return 5000;
    }
    inputContext.findStreamInfo(ec);
    if (ec) {
        return 5000;
    }

    const auto duration = inputContext.duration();
    if (!duration.isNoPts()) {
        const qint64 durationMs = static_cast<qint64>(duration.seconds() * 1000.0);
        if (durationMs > 0) {
            return durationMs;
        }
    }
    return 5000;
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

void TimelineEditor::addClipFromDrop(const QString &path, qint64 startMs, int preferredTrackIndex)
{
    if (path.isEmpty()) {
        return;
    }

    const qint64 durationMs = defaultDurationForPath(path);
    const bool audio = hasAudioForPath(path);
    const bool video = hasVideoForPath(path);
    const int linkedGroup = (video && audio) ? nextLinkedGroupId++ : -1;
    const auto pickTrack = [&](TrackType type) -> int {
        if (isTrackIndexValid(preferredTrackIndex) && tracks[static_cast<size_t>(preferredTrackIndex)].type == type) {
            return preferredTrackIndex;
        }
        int idx = maxTrackOfType(type);
        if (idx >= 0) {
            return idx;
        }
        return 0;
    };

    if (video) {
        ClipEvent v;
        v.id = nextClipId++;
        v.linkedGroupId = linkedGroup;
        v.mediaType = TrackType::Video;
        v.trackIndex = pickTrack(TrackType::Video);
        v.startMs = startMs;
        v.durationMs = durationMs;
        v.sourceInMs = 0;
        v.sourceDurationMs = durationMs;
        v.sourcePath = path;
        v.color = colorFromPath(path + ":v");
        clips.push_back(v);
        selectedClipId = v.id;
    }

    if (audio) {
        ClipEvent a;
        a.id = nextClipId++;
        a.linkedGroupId = linkedGroup;
        a.mediaType = TrackType::Audio;
        a.trackIndex = pickTrack(TrackType::Audio);
        a.startMs = startMs;
        a.durationMs = durationMs;
        a.sourceInMs = 0;
        a.sourceDurationMs = durationMs;
        a.sourcePath = path;
        a.color = colorFromPath(path + ":a");
        clips.push_back(a);
        if (!video) {
            selectedClipId = a.id;
        }
    }

    setCursorMs(startMs);
    emitOverlayInfo();
    refreshTimelineRangeSignals();
    update();
}

void TimelineEditor::emitOverlayInfo()
{
    QString activeVideoName = "-";
    int activeAudioCount = 0;

    for (const auto &clip : clips) {
        const bool active = cursorPositionMs >= clip.startMs &&
                            cursorPositionMs < (clip.startMs + clip.durationMs);
        if (!active || !isTrackIndexValid(clip.trackIndex)) {
            continue;
        }
        if (clip.mediaType == TrackType::Video) {
            activeVideoName = QFileInfo(clip.sourcePath).fileName();
        } else {
            ++activeAudioCount;
        }
    }

    QString regionText = "Region: -";
    if (hasRegionSelection && regionEndMs > regionStartMs) {
        regionText = QString("Region: %1s-%2s")
                         .arg(regionStartMs / 1000.0, 0, 'f', 2)
                         .arg(regionEndMs / 1000.0, 0, 'f', 2);
    }
    const QString text = QString("Overlay @ %1s | Video: %2 | Audio layers: %3 | %4")
                             .arg(cursorPositionMs / 1000.0, 0, 'f', 2)
                             .arg(activeVideoName)
                             .arg(activeAudioCount)
                             .arg(regionText + (loopRegionEnabled ? " (loop)" : ""));
    if (text != lastOverlayInfo) {
        lastOverlayInfo = text;
        emit overlayInfoChanged(text);
    }
}
