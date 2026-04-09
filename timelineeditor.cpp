#include "timelineeditor.h"

#include <QDragEnterEvent>
#include <QDropEvent>
#include <QContextMenuEvent>
#include <QFileInfo>
#include <QImageReader>
#include <QMenu>
#include <QMimeData>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QJsonArray>
#include <QJsonObject>
#include <QFontMetrics>
#include <QPointer>
#include <QUrl>
#include <QWheelEvent>
#include <QtConcurrent/QtConcurrentRun>
#include <QtGlobal>

#include <algorithm>
#include <chrono>
#include <cstring>
#include <cstdlib>
#include <limits>
#include <system_error>

#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/videorescaler.h>

namespace {
constexpr qint64 kSnapThresholdMs = 100;
constexpr qint64 kMinClipDurationMs = 80;
constexpr int kTrimHandlePx = 7;
constexpr int kEdgeScrollZonePx = 26;
constexpr qint64 kEdgeScrollStepMs = 80;
constexpr int kRulerTargetTickPx = 130;
constexpr int kCursorMagnetPx = 10;
constexpr int kRegionBoundaryHandlePx = 8;
const av::Rational kMillisecondsTimebase{1, 1000};

qint64 timestampToMs(const av::Timestamp &ts)
{
    if (ts.isNoPts()) {
        return -1;
    }
    return ts.timestamp(kMillisecondsTimebase);
}

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

QPoint eventPos(const QWheelEvent *event)
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

void TimelineEditor::setCursorMs(qint64 valueMs, bool ensureVisible)
{
    const qint64 bounded = std::max<qint64>(0, valueMs);
    if (bounded == cursorPositionMs) {
        return;
    }

    cursorPositionMs = bounded;
    if (ensureVisible) {
        ensureCursorVisible();
        emitOverlayInfo();
        refreshTimelineRangeSignals();
    }
    emit cursorChanged(cursorPositionMs);
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
    insertTrackAt(static_cast<int>(tracks.size()), TrackType::Video);
    setMinimumHeight(contentBottom() + bottomPadding());
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::addAudioTrack()
{
    insertTrackAt(static_cast<int>(tracks.size()), TrackType::Audio);
    setMinimumHeight(contentBottom() + bottomPadding());
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
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
    const int sourceGroupId = selected->linkedGroupId;
    const int rightGroupId = (sourceGroupId >= 0) ? nextLinkedGroupId++ : -1;
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
        right.linkedGroupId = rightGroupId;
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
    emit timelineContentChanged();
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
    const qint64 sourceDelta = newStart - clip->startMs;
    const qint64 newSourceIn = std::clamp(clip->sourceInMs + sourceDelta, 0LL, maxSourceIn);
    newStart = clip->startMs + (newSourceIn - clip->sourceInMs);
    if (newStart <= clip->startMs) {
        return false;
    }
    const qint64 delta = newStart - clip->startMs;
    clip->durationMs -= delta;
    clip->startMs = newStart;
    clip->sourceInMs = newSourceIn;
    if (clip->linkedGroupId >= 0) {
        for (int id : linkedClipIds(clip->id)) {
            if (id == clip->id) {
                continue;
            }
            ClipEvent *linked = findClipById(id);
            if (!linked) {
                continue;
            }
            linked->startMs += delta;
            linked->sourceInMs = std::max<qint64>(0, linked->sourceInMs + delta);
            linked->durationMs = std::max<qint64>(kMinClipDurationMs, linked->durationMs - delta);
        }
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
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
    const qint64 oldDuration = clip->durationMs;
    clip->durationMs = newEnd - clip->startMs;
    if (clip->linkedGroupId >= 0) {
        const qint64 deltaDur = clip->durationMs - oldDuration;
        for (int id : linkedClipIds(clip->id)) {
            if (id == clip->id) {
                continue;
            }
            ClipEvent *linked = findClipById(id);
            if (!linked) {
                continue;
            }
            linked->durationMs = std::max<qint64>(kMinClipDurationMs, linked->durationMs + deltaDur);
        }
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
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
            emit timelineContentChanged();
            update();
            return true;
        }
        target += ((direction > 0) ? 1 : -1);
    }
    return false;
}

bool TimelineEditor::copySelectedToClipboard()
{
    clipboardClips.clear();
    const ClipEvent *selected = findClipById(selectedClipId);
    if (!selected) {
        return false;
    }
    const auto ids = linkedClipIds(selectedClipId);
    for (int id : ids) {
        const ClipEvent *clip = findClipById(id);
        if (clip) {
            clipboardClips.push_back(*clip);
        }
    }
    return !clipboardClips.empty();
}

bool TimelineEditor::pasteClipboardAtCursor()
{
    if (clipboardClips.empty()) {
        return false;
    }
    qint64 sourceMinStart = clipboardClips.front().startMs;
    for (const auto &clip : clipboardClips) {
        sourceMinStart = std::min(sourceMinStart, clip.startMs);
    }
    int newGroup = -1;
    if (clipboardClips.size() > 1) {
        newGroup = nextLinkedGroupId++;
    }
    int firstNewId = -1;
    for (const auto &src : clipboardClips) {
        ClipEvent c = src;
        c.id = nextClipId++;
        c.startMs = cursorPositionMs + (src.startMs - sourceMinStart);
        if (!isTrackIndexValid(c.trackIndex)) {
            c.trackIndex = ensureDropTrackForType(c.mediaType, contentBottom() + 20);
        }
        c.linkedGroupId = (newGroup >= 0) ? newGroup : -1;
        clips.push_back(c);
        if (firstNewId < 0) {
            firstNewId = c.id;
        }
    }
    selectedClipId = firstNewId;
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
    return true;
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

bool TimelineEditor::deleteSelectedClip()
{
    if (selectedClipId < 0) {
        return false;
    }

    const auto ids = linkedClipIds(selectedClipId);
    const size_t before = clips.size();
    clips.erase(std::remove_if(clips.begin(), clips.end(), [&](const ClipEvent &clip) {
                   return std::find(ids.begin(), ids.end(), clip.id) != ids.end();
               }),
               clips.end());
    if (clips.size() == before) {
        return false;
    }
    selectedClipId = -1;
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
    return true;
}

bool TimelineEditor::magnetizeSelectedClip()
{
    ClipEvent *selected = findClipById(selectedClipId);
    if (!selected) {
        return false;
    }

    ClipEvent *best = nullptr;
    qint64 bestDistance = std::numeric_limits<qint64>::max();
    for (auto &clip : clips) {
        if (clip.id == selected->id) {
            continue;
        }
        if (clip.mediaType == selected->mediaType) {
            continue;
        }
        if (clip.sourcePath != selected->sourcePath) {
            continue;
        }
        const qint64 d = std::llabs(clip.startMs - selected->startMs);
        if (d < bestDistance) {
            bestDistance = d;
            best = &clip;
        }
    }
    if (!best || bestDistance > kSnapThresholdMs * 2) {
        return false;
    }

    const qint64 targetStart = snapTimeMs(selected->startMs);
    best->startMs = targetStart;
    selected->startMs = targetStart;
    best->durationMs = selected->durationMs;
    best->sourceInMs = selected->sourceInMs;

    int group = selected->linkedGroupId;
    if (group < 0 && best->linkedGroupId >= 0) {
        group = best->linkedGroupId;
    }
    if (group < 0) {
        group = nextLinkedGroupId++;
    }
    selected->linkedGroupId = group;
    best->linkedGroupId = group;

    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
    return true;
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

void TimelineEditor::setZoomFactor(double value)
{
    setZoomFactor(value, cursorPositionMs);
}

void TimelineEditor::setZoomFactor(double value, qint64 anchorTimeMs)
{
    const double bounded = std::clamp(value, 0.01, 32.0);
    if (qFuzzyCompare(zoomLevel, bounded)) {
        return;
    }
    const qint64 oldVisible = contentVisibleMs();
    const qint64 oldViewStart = timelineViewStartMs;
    const qint64 anchor = std::max<qint64>(0, anchorTimeMs);
    const double anchorRatio = (oldVisible > 0)
        ? static_cast<double>(anchor - oldViewStart) / static_cast<double>(oldVisible)
        : 0.5;
    zoomLevel = bounded;
    const qint64 newVisible = contentVisibleMs();
    timelineViewStartMs = std::max<qint64>(0, anchor - static_cast<qint64>(anchorRatio * newVisible));
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    update();
}

double TimelineEditor::zoomFactor() const
{
    return zoomLevel;
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

std::vector<TimelineEditor::RenderClip> TimelineEditor::renderClips() const
{
    std::vector<RenderClip> result;
    result.reserve(clips.size());
    for (const auto &clip : clips) {
        if (!isTrackIndexValid(clip.trackIndex)) {
            continue;
        }
        RenderClip c;
        c.sourcePath = clip.sourcePath;
        c.startMs = clip.startMs;
        c.durationMs = clip.durationMs;
        c.sourceInMs = clip.sourceInMs;
        c.trackIndex = clip.trackIndex;
        c.isVideo = (clip.mediaType == TrackType::Video);
        c.isAudio = (clip.mediaType == TrackType::Audio);
        result.push_back(c);
    }
    return result;
}

QString TimelineEditor::formatTimelineTime(qint64 timeMs) const
{
    const qint64 positive = std::max<qint64>(0, timeMs);
    const qint64 totalSeconds = positive / 1000;
    const qint64 msPart = positive % 1000;
    const qint64 hours = totalSeconds / 3600;
    const qint64 minutes = (totalSeconds / 60) % 60;
    const qint64 seconds = totalSeconds % 60;
    const int fps = std::max(1, timelineFps);
    const int frame = static_cast<int>((msPart * fps) / 1000);
    return QString("%1:%2:%3.%4 F%5")
        .arg(hours, 2, 10, QChar('0'))
        .arg(minutes, 2, 10, QChar('0'))
        .arg(seconds, 2, 10, QChar('0'))
        .arg(msPart, 3, 10, QChar('0'))
        .arg(frame, 2, 10, QChar('0'));
}

void TimelineEditor::setTimelineFps(int fps)
{
    const int bounded = std::max(1, fps);
    if (bounded == timelineFps) {
        return;
    }
    timelineFps = bounded;
    emitOverlayInfo();
    update();
}

void TimelineEditor::setCacheCoverage(const QHash<QString, QVector<QPair<double, double>>> &clipSegments,
                                      const QVector<QPair<double, double>> &timelineSegments)
{
    cacheSegmentsByClip = clipSegments;
    timelineCacheSegments = timelineSegments;
    update();
}

QJsonObject TimelineEditor::toJson() const
{
    QJsonObject root;
    root.insert("version", 1);
    root.insert("nextClipId", nextClipId);
    root.insert("nextLinkedGroupId", nextLinkedGroupId);
    root.insert("cursorMs", static_cast<double>(cursorPositionMs));
    root.insert("selectedClipId", selectedClipId);
    root.insert("timelineViewStartMs", static_cast<double>(timelineViewStartMs));
    root.insert("zoomLevel", zoomLevel);
    root.insert("timelineFps", timelineFps);
    root.insert("loopRegionEnabled", loopRegionEnabled);
    root.insert("hasRegionSelection", hasRegionSelection);
    root.insert("regionStartMs", static_cast<double>(regionStartMs));
    root.insert("regionEndMs", static_cast<double>(regionEndMs));

    QJsonArray tracksArray;
    for (const auto &track : tracks) {
        QJsonObject trackObject;
        trackObject.insert("type", track.type == TrackType::Video ? "video" : "audio");
        trackObject.insert("name", track.name);
        tracksArray.push_back(trackObject);
    }
    root.insert("tracks", tracksArray);

    QJsonArray clipsArray;
    for (const auto &clip : clips) {
        QJsonObject clipObject;
        clipObject.insert("id", clip.id);
        clipObject.insert("linkedGroupId", clip.linkedGroupId);
        clipObject.insert("mediaType", clip.mediaType == TrackType::Video ? "video" : "audio");
        clipObject.insert("trackIndex", clip.trackIndex);
        clipObject.insert("startMs", static_cast<double>(clip.startMs));
        clipObject.insert("durationMs", static_cast<double>(clip.durationMs));
        clipObject.insert("sourceInMs", static_cast<double>(clip.sourceInMs));
        clipObject.insert("sourceDurationMs", static_cast<double>(clip.sourceDurationMs));
        clipObject.insert("sourcePath", clip.sourcePath);

        QJsonObject colorObject;
        colorObject.insert("r", clip.color.red());
        colorObject.insert("g", clip.color.green());
        colorObject.insert("b", clip.color.blue());
        colorObject.insert("a", clip.color.alpha());
        clipObject.insert("color", colorObject);

        clipsArray.push_back(clipObject);
    }
    root.insert("clips", clipsArray);

    QJsonArray markersArray;
    for (qint64 marker : markersMs) {
        markersArray.push_back(static_cast<double>(marker));
    }
    root.insert("markersMs", markersArray);

    return root;
}

bool TimelineEditor::fromJson(const QJsonObject &root)
{
    if (root.isEmpty()) {
        return false;
    }

    std::vector<TrackLane> loadedTracks;
    const QJsonArray tracksArray = root.value("tracks").toArray();
    loadedTracks.reserve(static_cast<size_t>(tracksArray.size()));
    for (const auto &trackValue : tracksArray) {
        const QJsonObject trackObject = trackValue.toObject();
        const QString typeString = trackObject.value("type").toString();
        TrackType type = TrackType::Video;
        if (typeString == "audio") {
            type = TrackType::Audio;
        }
        QString name = trackObject.value("name").toString();
        if (name.isEmpty()) {
            const int suffix = static_cast<int>(loadedTracks.size()) + 1;
            name = QString("%1%2").arg(type == TrackType::Video ? "V" : "A").arg(suffix);
        }
        loadedTracks.push_back(TrackLane{type, name});
    }
    if (loadedTracks.empty()) {
        loadedTracks.push_back(TrackLane{TrackType::Video, "V1"});
        loadedTracks.push_back(TrackLane{TrackType::Audio, "A1"});
    }

    std::vector<ClipEvent> loadedClips;
    const QJsonArray clipsArray = root.value("clips").toArray();
    loadedClips.reserve(static_cast<size_t>(clipsArray.size()));
    int maxClipId = 0;
    int maxLinkedGroupId = 0;
    for (const auto &clipValue : clipsArray) {
        const QJsonObject clipObject = clipValue.toObject();
        ClipEvent clip;
        clip.id = clipObject.value("id").toInt();
        clip.linkedGroupId = clipObject.value("linkedGroupId").toInt(-1);
        clip.mediaType = (clipObject.value("mediaType").toString() == "audio") ? TrackType::Audio : TrackType::Video;
        clip.trackIndex = clipObject.value("trackIndex").toInt(0);
        clip.startMs = static_cast<qint64>(clipObject.value("startMs").toDouble(0.0));
        clip.durationMs = static_cast<qint64>(clipObject.value("durationMs").toDouble(0.0));
        clip.sourceInMs = static_cast<qint64>(clipObject.value("sourceInMs").toDouble(0.0));
        clip.sourceDurationMs = static_cast<qint64>(clipObject.value("sourceDurationMs").toDouble(0.0));
        clip.sourcePath = clipObject.value("sourcePath").toString();
        const QJsonObject colorObject = clipObject.value("color").toObject();
        clip.color = QColor(colorObject.value("r").toInt(140),
                            colorObject.value("g").toInt(140),
                            colorObject.value("b").toInt(140),
                            colorObject.value("a").toInt(255));

        if (clip.id <= 0 || clip.durationMs <= 0 || clip.trackIndex < 0 ||
            clip.trackIndex >= static_cast<int>(loadedTracks.size()) || clip.sourcePath.isEmpty()) {
            continue;
        }
        if (clip.sourceDurationMs <= 0) {
            clip.sourceDurationMs = clip.durationMs;
        }
        clip.startMs = std::max<qint64>(0, clip.startMs);
        clip.sourceInMs = std::max<qint64>(0, clip.sourceInMs);
        loadedClips.push_back(clip);
        maxClipId = std::max(maxClipId, clip.id);
        maxLinkedGroupId = std::max(maxLinkedGroupId, clip.linkedGroupId);
    }

    std::vector<qint64> loadedMarkers;
    const QJsonArray markersArray = root.value("markersMs").toArray();
    loadedMarkers.reserve(static_cast<size_t>(markersArray.size()));
    for (const auto &markerValue : markersArray) {
        const qint64 marker = std::max<qint64>(0, static_cast<qint64>(markerValue.toDouble(0.0)));
        loadedMarkers.push_back(marker);
    }
    std::sort(loadedMarkers.begin(), loadedMarkers.end());

    tracks = std::move(loadedTracks);
    clips = std::move(loadedClips);
    markersMs = std::move(loadedMarkers);
    clipboardClips.clear();
    previewCache.clear();
    pendingPreviewKeys.clear();
    peaksCache.clear();
    cacheSegmentsByClip.clear();
    timelineCacheSegments.clear();

    nextClipId = std::max(root.value("nextClipId").toInt(maxClipId + 1), maxClipId + 1);
    nextLinkedGroupId = std::max(root.value("nextLinkedGroupId").toInt(maxLinkedGroupId + 1), maxLinkedGroupId + 1);
    cursorPositionMs = std::max<qint64>(0, static_cast<qint64>(root.value("cursorMs").toDouble(0.0)));
    selectedClipId = root.value("selectedClipId").toInt(-1);
    timelineViewStartMs = std::max<qint64>(0, static_cast<qint64>(root.value("timelineViewStartMs").toDouble(0.0)));
    zoomLevel = std::clamp(root.value("zoomLevel").toDouble(1.0), 0.01, 32.0);
    timelineFps = std::max(1, root.value("timelineFps").toInt(30));
    loopRegionEnabled = root.value("loopRegionEnabled").toBool(false);
    hasRegionSelection = root.value("hasRegionSelection").toBool(false);
    regionStartMs = std::max<qint64>(0, static_cast<qint64>(root.value("regionStartMs").toDouble(0.0)));
    regionEndMs = std::max(regionStartMs, static_cast<qint64>(root.value("regionEndMs").toDouble(0.0)));

    if (selectedClipId >= 0 && !findClipById(selectedClipId)) {
        selectedClipId = -1;
    }
    if (!hasRegionSelection) {
        regionStartMs = 0;
        regionEndMs = 0;
    }

    setMinimumHeight(contentBottom() + bottomPadding());
    emit loopEnabledChanged(loopRegionEnabled);
    emitRegionChanged();
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    emit cursorChanged(cursorPositionMs);
    update();
    return true;
}

void TimelineEditor::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);

    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.fillRect(rect(), QColor(18, 18, 18));

    const int paintBottom = height() - 4;
    const QRect rulerRect(timelineLeft(), rulerTop(), timelineRight() - timelineLeft(), rulerHeight());
    const QRect timelineCacheBarRect(timelineLeft(), std::max(2, rulerTop() - 8), timelineRight() - timelineLeft(), 5);
    for (const auto &segment : timelineCacheSegments) {
        const double a = std::clamp(segment.first, 0.0, 1.0);
        const double b = std::clamp(segment.second, 0.0, 1.0);
        if (b <= a) {
            continue;
        }
        const int x1 = timelineCacheBarRect.left() + static_cast<int>(a * timelineCacheBarRect.width());
        const int x2 = timelineCacheBarRect.left() + static_cast<int>(b * timelineCacheBarRect.width());
        p.fillRect(QRect(x1, timelineCacheBarRect.top(), std::max(1, x2 - x1), timelineCacheBarRect.height()),
                   QColor(88, 198, 255));
    }
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
    const qint64 tickStepMs = timelineTickStepMs();
    const qint64 tickStart = (timelineViewStartMs / tickStepMs) * tickStepMs;
    for (qint64 t = std::max<qint64>(0, tickStart - tickStepMs); t <= viewEndMs + tickStepMs; t += tickStepMs) {
        const int x = timeToX(t);
        p.setPen(QColor(70, 70, 70));
        p.drawLine(x, rulerTop(), x, paintBottom);
        p.setPen(QColor(170, 170, 170));
        QString tickLabel = formatTimelineTime(t);
        if (tickStepMs >= 1000 && tickStepMs < 60000) {
            tickLabel = tickLabel.mid(3, 5);
        } else if (tickStepMs >= 60000) {
            tickLabel = tickLabel.left(8);
        }
        p.drawText(x + 2, rulerTop() + 14, tickLabel);
    }

    if (hasRegionSelection && regionEndMs > regionStartMs) {
        const int x1 = timeToX(regionStartMs);
        const int x2 = timeToX(regionEndMs);
        const QRect regionRect(std::min(x1, x2), rulerTop(), std::abs(x2 - x1), paintBottom - rulerTop());
        p.fillRect(regionRect, QColor(84, 124, 255, 42));
        p.setPen(QPen(QColor(102, 152, 255), 2));
        p.drawLine(x1, rulerTop(), x1, paintBottom);
        p.drawLine(x2, rulerTop(), x2, paintBottom);
    }

    for (qint64 marker : markersMs) {
        const int x = timeToX(marker);
        p.setPen(QColor(255, 195, 0));
        p.drawLine(x, rulerTop(), x, paintBottom);
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

        if (clip.mediaType == TrackType::Video) {
            const int thumbW = std::max(28, r.height() * 16 / 9);
            const int innerLeft = r.left() + 2;
            const int innerRight = r.right() - 2;
            const int innerWidth = std::max(1, innerRight - innerLeft);
            for (int x = innerLeft; x < innerRight; x += thumbW) {
                const QRect thumbRect(x, r.top() + 2, std::min(thumbW, innerRight - x), r.height() - 4);
                const int centerX = thumbRect.left() + (thumbRect.width() / 2);
                const double rel = std::clamp(static_cast<double>(centerX - innerLeft) / innerWidth, 0.0, 1.0);
                const qint64 sourceOffsetMs =
                    static_cast<qint64>(std::llround(rel * std::max<qint64>(0, clip.durationMs - 1)));
                const QImage preview = cachedVideoPreview(clip.sourcePath, clip.sourceInMs + sourceOffsetMs);
                if (!preview.isNull()) {
                    p.drawImage(thumbRect, preview);
                }
            }
        } else {
            const int usable = std::max(1, r.width() - 6);
            const auto peaks =
                cachedAudioPeaks(clip.sourcePath, clip.sourceInMs, clip.durationMs, clip.sourceDurationMs, usable);
            if (!peaks.empty()) {
                p.setPen(QColor(210, 225, 255, 220));
                const int midY = r.center().y();
                for (int x = 0; x < usable; ++x) {
                    const int idx = std::min<int>(x, static_cast<int>(peaks.size()) - 1);
                    const int amp = std::clamp(peaks[static_cast<size_t>(idx)], 1, (r.height() / 2) - 2);
                    p.drawLine(r.left() + 3 + x, midY - amp, r.left() + 3 + x, midY + amp);
                }
            }
        }
        if (shouldDrawClipLabel(clip)) {
            const QString fileName = QFileInfo(clip.sourcePath).fileName();
            const QFontMetrics fm(p.font());
            const QString shownName = fm.elidedText(fileName, Qt::ElideMiddle, r.width() - 14);
            const int textWidth = fm.horizontalAdvance(shownName);
            const int textHeight = fm.height();
            const QRect textRect(r.left() + 5, r.top() + 4, textWidth, textHeight);
            const QRect bgRect = textRect.adjusted(-3, -2, 4, 2);
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(0, 0, 0, 150));
            p.drawRoundedRect(bgRect, 3, 3);
            p.setPen(QColor(240, 240, 240));
            p.drawText(textRect, Qt::AlignLeft | Qt::AlignVCenter, shownName);
        }
        if (clip.id == selectedClipId) {
            p.setPen(QColor(255, 255, 255));
            p.drawLine(r.left() + 2, r.top() + 2, r.left() + 2, r.bottom() - 2);
            p.drawLine(r.right() - 2, r.top() + 2, r.right() - 2, r.bottom() - 2);
        }

        const auto segments = cacheSegmentsByClip.value(clipCoverageKey(clip));
        const QRect barRect(r.left() + 3, r.top() - 6, std::max(6, r.width() - 6), 4);
        const QColor barColor = (clip.mediaType == TrackType::Audio) ? QColor(114, 214, 255) : QColor(82, 226, 124);
        for (const auto &segment : segments) {
            const double a = std::clamp(segment.first, 0.0, 1.0);
            const double b = std::clamp(segment.second, 0.0, 1.0);
            if (b <= a) {
                continue;
            }
            const int x1 = barRect.left() + static_cast<int>(a * barRect.width());
            const int x2 = barRect.left() + static_cast<int>(b * barRect.width());
            p.fillRect(QRect(x1, barRect.top(), std::max(1, x2 - x1), barRect.height()), barColor);
        }
    }

    const int cursorX = timeToX(cursorPositionMs);
    p.setPen(QPen(QColor(255, 60, 60, 120), 6));
    p.drawLine(cursorX, rulerTop(), cursorX, paintBottom);
    p.setPen(QPen(QColor(255, 100, 100), 2));
    p.drawLine(cursorX, rulerTop(), cursorX, paintBottom);

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

    const bool inTimelineX = pos.x() >= timelineLeft() && pos.x() <= timelineRight();
    selectedClipId = clipIdAt(pos);
    const qint64 pointerMs = xToTimeMs(pos.x());
    setCursorMs(snapTimeMs(pointerMs));

    dragMode = DragMode::None;
    dragClipId = selectedClipId;
    dragChangedTimelineContent = false;
    if (selectedClipId >= 0) {
        ClipEvent *clip = findClipById(selectedClipId);
        if (clip) {
            dragAnchorMs = pointerMs;
            dragStartPointerMs = pointerMs;
            dragLastPointerMs = pointerMs;
            dragOriginalStartMs = clip->startMs;
            dragOriginalTrackIndex = clip->trackIndex;
            trimOriginalDurationMs = clip->durationMs;
            trimOriginalSourceInMs = clip->sourceInMs;
            dragInitialStarts.clear();
            dragInitialDurations.clear();
            dragInitialSourceIns.clear();
            for (int id : linkedClipIds(clip->id)) {
                ClipEvent *linked = findClipById(id);
                if (!linked) {
                    continue;
                }
                dragInitialStarts.insert(id, linked->startMs);
                dragInitialDurations.insert(id, linked->durationMs);
                dragInitialSourceIns.insert(id, linked->sourceInMs);
            }
            if (isNearClipStartEdge(*clip, pos.x())) {
                dragMode = DragMode::TrimStart;
            } else if (isNearClipEndEdge(*clip, pos.x())) {
                dragMode = DragMode::TrimEnd;
            } else {
                dragMode = DragMode::MoveClip;
            }
        }
    } else if (inTimelineX && pos.y() >= rulerTop() && pos.y() <= (rulerTop() + rulerHeight())) {
        if (hasRegionSelection && isNearRegionStartBoundary(pos.x())) {
            dragMode = DragMode::MoveRegionStart;
        } else if (hasRegionSelection && isNearRegionEndBoundary(pos.x())) {
            dragMode = DragMode::MoveRegionEnd;
        } else {
            dragMode = DragMode::ScrubCursor;
        }
    } else if (inTimelineX && pos.y() >= (rulerTop() + rulerHeight()) && pos.y() <= (height() - bottomPadding())) {
        dragMode = DragMode::RegionSelect;
        regionAnchorMs = snapTimeMs(pointerMs);
        setRegionBounds(regionAnchorMs, regionAnchorMs);
    }

    update();
}

void TimelineEditor::mouseDoubleClickEvent(QMouseEvent *event)
{
    if (event->button() != Qt::LeftButton) {
        QWidget::mouseDoubleClickEvent(event);
        return;
    }
    const QPoint pos = eventPos(event);
    const int id = clipIdAt(pos);
    if (id < 0) {
        QWidget::mouseDoubleClickEvent(event);
        return;
    }

    selectedClipId = id;
    const auto ids = linkedClipIds(id);
    qint64 startMs = std::numeric_limits<qint64>::max();
    qint64 endMs = 0;
    for (int linkedId : ids) {
        const ClipEvent *clip = findClipById(linkedId);
        if (!clip) {
            continue;
        }
        startMs = std::min(startMs, clip->startMs);
        endMs = std::max(endMs, clip->startMs + clip->durationMs);
    }
    if (startMs != std::numeric_limits<qint64>::max() && endMs > startMs) {
        setRegionBounds(startMs, endMs);
        setCursorMs(startMs, true);
    }
    emitOverlayInfo();
    update();
    event->accept();
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
        } else if (pos.x() >= timelineLeft() && pos.x() <= timelineRight() &&
                   pos.y() >= rulerTop() && pos.y() <= (rulerTop() + rulerHeight()) &&
                   hasRegionSelection &&
                   (isNearRegionStartBoundary(pos.x()) || isNearRegionEndBoundary(pos.x()))) {
            setCursor(Qt::SizeHorCursor);
        } else {
            unsetCursor();
        }
        QWidget::mouseMoveEvent(event);
        return;
    }

    const qint64 pointerMsRaw = adjustedDragPointerMs(pos);
    const qint64 pointerMs = snapTimeMs(pointerMsRaw);
    setCursorMs(pointerMs);
    ClipEvent *clip = findClipById(dragClipId);
    if (dragMode == DragMode::ScrubCursor) {
        setCursorMs(pointerMs);
    } else if (dragMode == DragMode::MoveClip && clip) {
        dragChangedTimelineContent = true;
        qint64 desiredDelta = pointerMs - dragStartPointerMs;
        qint64 minStart = std::numeric_limits<qint64>::max();
        for (auto it = dragInitialStarts.constBegin(); it != dragInitialStarts.constEnd(); ++it) {
            minStart = std::min(minStart, static_cast<qint64>(it.value()));
        }
        if (minStart == std::numeric_limits<qint64>::max()) {
            minStart = 0;
        }
        desiredDelta = std::max<qint64>(desiredDelta, -minStart);
        for (auto it = dragInitialStarts.constBegin(); it != dragInitialStarts.constEnd(); ++it) {
            ClipEvent *linked = findClipById(it.key());
            if (!linked) {
                continue;
            }
            linked->startMs = std::max<qint64>(0, static_cast<qint64>(it.value()) + desiredDelta);
        }
        const int targetTrack = trackIndexAtY(pos.y());
        if (targetTrack >= 0 && tracks[static_cast<size_t>(targetTrack)].type == clip->mediaType) {
            clip->trackIndex = targetTrack;
        }
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::TrimStart && clip) {
        dragChangedTimelineContent = true;
        const qint64 originalStart = dragInitialStarts.value(clip->id, dragOriginalStartMs);
        const qint64 originalDuration = dragInitialDurations.value(clip->id, trimOriginalDurationMs);
        const qint64 originalSourceIn = dragInitialSourceIns.value(clip->id, trimOriginalSourceInMs);
        const qint64 originalEnd = originalStart + originalDuration;
        const qint64 maxSourceIn = std::max<qint64>(0, clip->sourceDurationMs - kMinClipDurationMs);
        qint64 wantedStart = std::clamp(pointerMs, originalStart, originalEnd - kMinClipDurationMs);
        qint64 newSourceIn = std::clamp(originalSourceIn + (wantedStart - originalStart), 0LL, maxSourceIn);
        wantedStart = originalStart + (newSourceIn - originalSourceIn);
        clip->startMs = wantedStart;
        clip->sourceInMs = newSourceIn;
        clip->durationMs = originalEnd - wantedStart;
        if (clip->linkedGroupId >= 0) {
            const qint64 delta = wantedStart - originalStart;
            for (const int id : linkedClipIds(clip->id)) {
                if (id == clip->id) {
                    continue;
                }
                ClipEvent *linked = findClipById(id);
                if (!linked) {
                    continue;
                }
                const qint64 linkedStart = dragInitialStarts.value(id, linked->startMs);
                const qint64 linkedDuration = dragInitialDurations.value(id, linked->durationMs);
                const qint64 linkedSourceIn = dragInitialSourceIns.value(id, linked->sourceInMs);
                linked->startMs = linkedStart + delta;
                linked->sourceInMs = std::max<qint64>(0, linkedSourceIn + delta);
                linked->durationMs = std::max<qint64>(kMinClipDurationMs, linkedDuration - delta);
            }
        }
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::TrimEnd && clip) {
        dragChangedTimelineContent = true;
        const qint64 originalStart = dragInitialStarts.value(clip->id, dragOriginalStartMs);
        const qint64 originalDuration = dragInitialDurations.value(clip->id, trimOriginalDurationMs);
        const qint64 originalSourceIn = dragInitialSourceIns.value(clip->id, trimOriginalSourceInMs);
        const qint64 minEnd = originalStart + kMinClipDurationMs;
        const qint64 maxEnd = originalStart + std::max<qint64>(kMinClipDurationMs, clip->sourceDurationMs - originalSourceIn);
        const qint64 wantedEnd = std::clamp(pointerMs, minEnd, std::max(minEnd, maxEnd));
        const qint64 oldDuration = originalDuration;
        clip->durationMs = wantedEnd - originalStart;
        if (clip->linkedGroupId >= 0) {
            const qint64 deltaDur = clip->durationMs - oldDuration;
            for (const int id : linkedClipIds(clip->id)) {
                if (id == clip->id) {
                    continue;
                }
                ClipEvent *linked = findClipById(id);
                if (!linked) {
                    continue;
                }
                const qint64 linkedDuration = dragInitialDurations.value(id, linked->durationMs);
                linked->durationMs = std::max<qint64>(kMinClipDurationMs, linkedDuration + deltaDur);
            }
        }
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::MoveRegionStart) {
        if (hasRegionSelection) {
            setRegionBounds(pointerMs, regionEndMs);
        }
    } else if (dragMode == DragMode::MoveRegionEnd) {
        if (hasRegionSelection) {
            setRegionBounds(regionStartMs, pointerMs);
        }
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
    dragInitialStarts.clear();
    dragInitialDurations.clear();
    dragInitialSourceIns.clear();
    if (dragChangedTimelineContent) {
        dragChangedTimelineContent = false;
        emit timelineContentChanged();
    }
    unsetCursor();
    QWidget::mouseReleaseEvent(event);
}

void TimelineEditor::contextMenuEvent(QContextMenuEvent *event)
{
    const int id = clipIdAt(event->pos());
    if (id < 0) {
        QWidget::contextMenuEvent(event);
        return;
    }
    selectedClipId = id;
    QMenu menu(this);
    QAction *unlinkAction = nullptr;
    const ClipEvent *clip = findClipById(id);
    if (clip && clip->linkedGroupId >= 0) {
        unlinkAction = menu.addAction("Unmagnet (Ungroup A/V)");
    }
    QAction *copyAction = menu.addAction("Copy");
    QAction *deleteAction = menu.addAction("Delete (Del)");
    QAction *magnetizeAction = menu.addAction("Magnetize A/V");
    QAction *splitAction = menu.addAction("Cut at Cursor (S)");
    QAction *chosen = menu.exec(event->globalPos());
    if (!chosen) {
        return;
    }
    if (chosen == unlinkAction) {
        unlinkClipGroup(id);
    } else if (chosen == copyAction) {
        copySelectedToClipboard();
    } else if (chosen == deleteAction) {
        deleteSelectedClip();
    } else if (chosen == magnetizeAction) {
        magnetizeSelectedClip();
    } else if (chosen == splitAction) {
        splitSelectedClipAtCursor();
    }
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
    const QPoint pos = eventPos(event);
    const qint64 dropStartMs = snapTimeMs(xToTimeMs(pos.x()));
    const int dropY = pos.y();

    if (event->mimeData()->hasUrls()) {
        for (const auto &url : event->mimeData()->urls()) {
            if (url.isLocalFile()) {
                addClipFromDrop(url.toLocalFile(), dropStartMs, dropY);
            }
        }
        event->acceptProposedAction();
        return;
    }

    if (event->mimeData()->hasText()) {
        addClipFromDrop(event->mimeData()->text().trimmed(), dropStartMs, dropY);
        event->acceptProposedAction();
        return;
    }

    QWidget::dropEvent(event);
}

void TimelineEditor::wheelEvent(QWheelEvent *event)
{
    if (event->modifiers().testFlag(Qt::ControlModifier)) {
        const QPoint delta = event->angleDelta();
        if (!delta.isNull()) {
            const double factor = (delta.y() > 0 || delta.x() > 0) ? 1.12 : (1.0 / 1.12);
            const QPoint pos = eventPos(event);
            const bool onTimeline = pos.x() >= timelineLeft() && pos.x() <= timelineRight();
            const qint64 anchor = onTimeline ? xToTimeMs(pos.x()) : cursorPositionMs;
            setZoomFactor(zoomLevel * factor, anchor);
        }
        event->accept();
        return;
    }

    const QPoint delta = event->angleDelta();
    const int dominant = (std::abs(delta.x()) > std::abs(delta.y())) ? delta.x() : delta.y();
    if (dominant != 0) {
        const qint64 shiftMs = static_cast<qint64>(-dominant * (contentVisibleMs() / 2400.0));
        setViewStartMs(timelineViewStartMs + shiftMs);
        event->accept();
        return;
    }
    QWidget::wheelEvent(event);
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
    return 90.0 * zoomLevel;
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

int TimelineEditor::insertTrackAt(int index, TrackType type)
{
    index = std::clamp(index, 0, static_cast<int>(tracks.size()));
    for (auto &clip : clips) {
        if (clip.trackIndex >= index) {
            ++clip.trackIndex;
        }
    }
    int sameTypeCount = 0;
    for (const auto &track : tracks) {
        if (track.type == type) {
            ++sameTypeCount;
        }
    }
    tracks.insert(tracks.begin() + index, TrackLane{type, QString("%1%2").arg(type == TrackType::Video ? "V" : "A").arg(sameTypeCount + 1)});
    return index;
}

int TimelineEditor::ensureDropTrackForType(TrackType type, int dropY)
{
    const int direct = trackIndexAtY(dropY);
    if (direct >= 0 && tracks[static_cast<size_t>(direct)].type == type) {
        return direct;
    }

    for (int i = 0; i + 1 < static_cast<int>(tracks.size()); ++i) {
        const int gapTop = trackTop(i) + trackHeight();
        const int gapBottom = trackTop(i + 1);
        if (dropY >= gapTop && dropY <= gapBottom) {
            return insertTrackAt(i + 1, type);
        }
    }

    if (tracks.empty()) {
        return insertTrackAt(0, type);
    }
    if (dropY < trackTop(0)) {
        return insertTrackAt(0, type);
    }
    if (dropY > contentBottom()) {
        return insertTrackAt(static_cast<int>(tracks.size()), type);
    }

    const int fallback = maxTrackOfType(type);
    if (fallback >= 0) {
        return fallback;
    }
    return insertTrackAt(static_cast<int>(tracks.size()), type);
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

bool TimelineEditor::hasMatchingLinkedVideo(const ClipEvent &clip) const
{
    if (clip.linkedGroupId < 0 || clip.mediaType != TrackType::Audio) {
        return false;
    }
    for (const auto &candidate : clips) {
        if (candidate.id == clip.id || candidate.linkedGroupId != clip.linkedGroupId) {
            continue;
        }
        if (candidate.mediaType != TrackType::Video) {
            continue;
        }
        if (candidate.sourcePath != clip.sourcePath) {
            continue;
        }
        if (candidate.startMs == clip.startMs &&
            candidate.durationMs == clip.durationMs &&
            candidate.sourceInMs == clip.sourceInMs) {
            return true;
        }
    }
    return false;
}

bool TimelineEditor::shouldDrawClipLabel(const ClipEvent &clip) const
{
    // Avoid duplicated names for normal magnetized A/V pairs.
    return !hasMatchingLinkedVideo(clip);
}

QString TimelineEditor::clipCoverageKey(const ClipEvent &clip) const
{
    return QStringLiteral("%1|%2|%3|%4")
        .arg(clip.sourcePath)
        .arg(clip.startMs)
        .arg(clip.durationMs)
        .arg(clip.sourceInMs);
}

bool TimelineEditor::isNearRegionStartBoundary(int x) const
{
    if (!hasRegionSelection || regionEndMs <= regionStartMs) {
        return false;
    }
    return std::abs(x - timeToX(regionStartMs)) <= kRegionBoundaryHandlePx;
}

bool TimelineEditor::isNearRegionEndBoundary(int x) const
{
    if (!hasRegionSelection || regionEndMs <= regionStartMs) {
        return false;
    }
    return std::abs(x - timeToX(regionEndMs)) <= kRegionBoundaryHandlePx;
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
    const qint64 margin = std::max<qint64>(visible / 7, 120);
    qint64 targetViewStart = timelineViewStartMs;
    if (cursorPositionMs < (timelineViewStartMs + margin)) {
        targetViewStart = std::max<qint64>(0, cursorPositionMs - margin);
    } else if (cursorPositionMs > (timelineViewStartMs + visible - margin)) {
        targetViewStart = std::max<qint64>(0, cursorPositionMs - (visible - margin));
    }
    targetViewStart = std::clamp<qint64>(targetViewStart, 0, maxViewStartMs());
    if (targetViewStart != timelineViewStartMs) {
        timelineViewStartMs = targetViewStart;
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
    const qint64 pxThresholdMs = static_cast<qint64>((kCursorMagnetPx / std::max(1.0, pixelsPerSecond())) * 1000.0);
    const qint64 dynamicThresholdMs = std::clamp<qint64>(pxThresholdMs, 12, kSnapThresholdMs);
    qint64 bestDistance = dynamicThresholdMs + 1;
    const auto consider = [&](qint64 v) {
        const qint64 d = std::llabs(v - candidateMs);
        if (d <= dynamicThresholdMs && d < bestDistance) {
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

qint64 TimelineEditor::adjustedDragPointerMs(const QPoint &pos, bool *scrolled)
{
    bool changed = false;
    if (dragMode != DragMode::None) {
        if (pos.x() < timelineLeft() + kEdgeScrollZonePx) {
            const qint64 before = timelineViewStartMs;
            setViewStartMs(timelineViewStartMs - kEdgeScrollStepMs);
            changed = changed || (before != timelineViewStartMs);
        } else if (pos.x() > timelineRight() - kEdgeScrollZonePx) {
            const qint64 before = timelineViewStartMs;
            setViewStartMs(timelineViewStartMs + kEdgeScrollStepMs);
            changed = changed || (before != timelineViewStartMs);
        }
    }
    if (scrolled) {
        *scrolled = changed;
    }
    return xToTimeMs(pos.x());
}

qint64 TimelineEditor::timelineTickStepMs() const
{
    const double desiredMs = (kRulerTargetTickPx / std::max(1.0, pixelsPerSecond())) * 1000.0;
    static const qint64 steps[] = {
        1, 2, 5, 10, 20, 50, 100, 200, 500,
        1000, 2000, 5000, 10000, 15000, 30000,
        60000, 120000, 300000, 600000, 1800000, 3600000
    };
    for (qint64 step : steps) {
        if (step >= desiredMs) {
            return step;
        }
    }
    return steps[sizeof(steps) / sizeof(steps[0]) - 1];
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

QImage TimelineEditor::cachedVideoPreview(const QString &path, qint64 sourceInMs)
{
    const qint64 bucketMs = std::max<qint64>(0, (sourceInMs / 120) * 120);
    const QString cacheKey = QString("%1::%2").arg(path, QString::number(bucketMs));
    const auto it = previewCache.constFind(cacheKey);
    if (it != previewCache.constEnd()) {
        return it.value();
    }

    if (pendingPreviewKeys.contains(cacheKey)) {
        return QImage();
    }
    pendingPreviewKeys.insert(cacheKey);

    QPointer<TimelineEditor> guard(this);
    QtConcurrent::run([guard, path, bucketMs, cacheKey]() {
        if (!guard) {
            return;
        }
        QImage img = guard->decodeVideoPreviewSync(path, bucketMs);
        const QImage scaled =
            img.scaled(320, 180, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
        if (!guard) {
            return;
        }
        QMetaObject::invokeMethod(
            guard.data(),
            [guard, cacheKey, scaled]() {
                if (!guard) {
                    return;
                }
                guard->previewCache.insert(cacheKey, scaled);
                guard->pendingPreviewKeys.remove(cacheKey);
                guard->update();
            },
            Qt::QueuedConnection);
    });

    return QImage();
}

QImage TimelineEditor::decodeVideoPreviewSync(const QString &path, qint64 bucketMs) const
{
    QImage img;
    if (isImagePath(path)) {
        img.load(path);
    } else {
        using namespace av;
        std::error_code ec;
        FormatContext input;
        input.openInput(path.toStdString(), ec);
        if (!ec) {
            input.findStreamInfo(ec);
        }
        if (!ec) {
            int videoStream = -1;
            Stream stream;
            for (size_t i = 0; i < input.streamsCount(); ++i) {
                auto st = input.stream(i);
                if (st.mediaType() == AVMEDIA_TYPE_VIDEO) {
                    videoStream = static_cast<int>(i);
                    stream = st;
                    break;
                }
            }
            if (!stream.isNull() && videoStream >= 0) {
                VideoDecoderContext dec(stream);
                Codec codec = findDecodingCodec(dec.raw()->codec_id);
                dec.setCodec(codec);
                dec.open(ec);
                if (!ec) {
                    const qint64 seekMs = std::max<qint64>(0, bucketMs - 700);
                    if (input.seekable()) {
                        input.seek(Timestamp(std::chrono::milliseconds(seekMs)),
                                   static_cast<size_t>(videoStream),
                                   false,
                                   ec);
                        if (ec) {
                            ec.clear();
                        }
                    }
                    VideoRescaler rescaler(dec.width(), dec.height(), AV_PIX_FMT_RGB24);
                    for (int tries = 0; tries < 256; ++tries) {
                        Packet pkt = input.readPacket(ec);
                        if (ec || !pkt) {
                            break;
                        }
                        if (pkt.streamIndex() != videoStream) {
                            continue;
                        }
                        VideoFrame frame = dec.decode(pkt, ec);
                        if (ec || frame.isNull() || !frame.isComplete()) {
                            ec.clear();
                            continue;
                        }
                        const qint64 decodedMs = timestampToMs(frame.pts());
                        const qint64 frameMs = (decodedMs >= 0) ? decodedMs : bucketMs;
                        if (frameMs < bucketMs) {
                            continue;
                        }
                        auto rgb = rescaler.rescale(frame);
                        img = QImage(rgb.width(), rgb.height(), QImage::Format_RGB888);
                        const int dstStride = img.bytesPerLine();
                        const int srcStride = rgb.raw()->linesize[0];
                        const int copyStride = std::min(dstStride, srcStride);
                        auto *src = rgb.data();
                        auto *dst = img.bits();
                        for (int y = 0; y < rgb.height(); ++y) {
                            std::memcpy(dst + y * dstStride, src + y * srcStride, copyStride);
                        }
                        break;
                    }
                }
            }
        }
        if (img.isNull()) {
            QImageReader reader(path);
            reader.setAutoTransform(true);
            img = reader.read();
        }
    }
    if (img.isNull()) {
        img = QImage(160, 90, QImage::Format_RGB32);
        img.fill(colorFromPath(path));
    }
    return img;
}

std::vector<int> TimelineEditor::cachedAudioPeaks(const QString &path,
                                                  qint64 sourceInMs,
                                                  qint64 durationMs,
                                                  qint64 sourceDurationMs,
                                                  int sampleCount)
{
    if (sampleCount <= 0) {
        return {};
    }

    const auto it = peaksCache.constFind(path);
    std::vector<int> basePeaks;
    if (it != peaksCache.constEnd()) {
        basePeaks = it.value();
    } else {
        basePeaks.reserve(1024);
        const uint base = qHash(path);
        for (int i = 0; i < 1024; ++i) {
            const uint v = qHash(QString::number(i) + path) ^ (base >> (i % 11));
            basePeaks.push_back(2 + static_cast<int>(v % 20));
        }
        peaksCache.insert(path, basePeaks);
    }
    if (basePeaks.empty()) {
        return {};
    }

    const qint64 safeDuration = std::max<qint64>(1, sourceDurationMs);
    const double startRatio =
        std::clamp(static_cast<double>(std::max<qint64>(0, sourceInMs)) / static_cast<double>(safeDuration), 0.0, 1.0);
    const double endRatioRaw = static_cast<double>(std::max<qint64>(0, sourceInMs + std::max<qint64>(1, durationMs))) /
                               static_cast<double>(safeDuration);
    const double endRatio = std::clamp(std::max(endRatioRaw, startRatio + 0.0001), 0.0, 1.0);
    const double span = std::max(0.0001, endRatio - startRatio);

    std::vector<int> sliced;
    sliced.reserve(static_cast<size_t>(sampleCount));
    for (int i = 0; i < sampleCount; ++i) {
        const double xRatio = (sampleCount == 1) ? 0.0 : (static_cast<double>(i) / (sampleCount - 1));
        const double srcRatio = std::clamp(startRatio + (xRatio * span), 0.0, 1.0);
        const int idx = std::clamp(static_cast<int>(srcRatio * (basePeaks.size() - 1)), 0, static_cast<int>(basePeaks.size()) - 1);
        sliced.push_back(basePeaks[static_cast<size_t>(idx)]);
    }
    return sliced;
}

std::vector<int> TimelineEditor::linkedClipIds(int clipId) const
{
    std::vector<int> ids;
    const ClipEvent *clip = findClipById(clipId);
    if (!clip) {
        return ids;
    }
    if (clip->linkedGroupId < 0) {
        ids.push_back(clipId);
        return ids;
    }
    for (const auto &it : clips) {
        if (it.linkedGroupId == clip->linkedGroupId) {
            ids.push_back(it.id);
        }
    }
    return ids;
}

void TimelineEditor::unlinkClipGroup(int clipId)
{
    const ClipEvent *clip = findClipById(clipId);
    if (!clip || clip->linkedGroupId < 0) {
        return;
    }
    const int groupId = clip->linkedGroupId;
    for (auto &it : clips) {
        if (it.linkedGroupId == groupId) {
            it.linkedGroupId = -1;
        }
    }
    emit timelineContentChanged();
    update();
}

void TimelineEditor::addClipFromDrop(const QString &path, qint64 startMs, int dropY)
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
        v.mediaType = TrackType::Video;
        v.trackIndex = ensureDropTrackForType(TrackType::Video, dropY);
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
        a.trackIndex = ensureDropTrackForType(TrackType::Audio, dropY);
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
    emit timelineContentChanged();
    update();
}

void TimelineEditor::emitOverlayInfo()
{
    int activeVideoCount = 0;
    int activeAudioCount = 0;

    for (const auto &clip : clips) {
        const bool active = cursorPositionMs >= clip.startMs &&
                            cursorPositionMs < (clip.startMs + clip.durationMs);
        if (!active || !isTrackIndexValid(clip.trackIndex)) {
            continue;
        }
        if (clip.mediaType == TrackType::Video) {
            ++activeVideoCount;
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
    const QString text = QString("Time: %1 | Zoom: %2x | Video layers: %3 | Audio layers: %4 | %5")
                             .arg(formatTimelineTime(cursorPositionMs))
                             .arg(zoomLevel, 0, 'f', 2)
                             .arg(activeVideoCount)
                             .arg(activeAudioCount)
                             .arg(regionText + (loopRegionEnabled ? " (loop)" : ""));
    if (text != lastOverlayInfo) {
        lastOverlayInfo = text;
        emit overlayInfoChanged(text);
    }
}
