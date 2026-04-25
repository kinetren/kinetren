#include "timelineeditor.h"
#include "burstcon.h"

#include <QDateTime>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QContextMenuEvent>
#include <QKeyEvent>
#include <QMimeData>
#include <QFileInfo>
#include <QMessageBox>
#include <QImageReader>
#include <QMenu>
#include <QMimeData>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QBrush>
#include <QLinearGradient>
#include <QJsonArray>
#include <QJsonObject>
#include <QFontMetrics>
#include <QPointer>
#include <QUrl>
#include <QWheelEvent>
#include <QResizeEvent>
#include <QEvent>
#include <QTimer>
#include <QHash>
#include <QList>
#include <QSet>
#include <QtConcurrent/QtConcurrentRun>
#include <QtGlobal>

#include <atomic>

#include <algorithm>
#include <cmath>
#include <chrono>
#include <unordered_set>
#include <cstring>
#include <cstdlib>
#include <limits>
#include <memory>
#include <system_error>

#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/videorescaler.h>

namespace {

std::atomic<int> gStripPreviewDecodeJobs{0};

/// Thumbnail sampling along source time: one thumb for short clips; step scales with on-screen width (zoom).
qint64 videoClipThumbnailSourceStepMs(qint64 clipDurationMs, int clipInnerWidthPx, int thumbWpx)
{
    constexpr qint64 kMinStep = 1000;
    constexpr qint64 kMaxStep = 15000;
    constexpr qint64 kShortSingleMax = 5000;
    if (clipDurationMs <= 0) {
        return 1;
    }
    if (clipDurationMs <= kShortSingleMax) {
        return clipDurationMs;
    }
    const int thumb = std::max(24, thumbWpx);
    const int slotss = std::clamp(clipInnerWidthPx / thumb, 1, 96);
    const qint64 rough = clipDurationMs / static_cast<qint64>(slotss);
    return std::clamp(rough, kMinStep, kMaxStep);
}

constexpr qint64 kSnapThresholdMs = 100;
// Allow near frame-accurate cuts (1 ms ≈ sub-frame at high zoom; copy/trim/split use this floor).
constexpr qint64 kMinClipDurationMs = 1;
constexpr int kTrimHandlePx = 7;
constexpr qint64 kEdgeScrollMinStepMs = 1;
constexpr qint64 kEdgeScrollMaxStepMs = 42;
/// How aggressively scroll speed ramps from inner zone (0) to screen edge (1). Higher = slower at first.
constexpr double kEdgeScrollSpeedGamma = 4.15;
/// Applied after gamma — each mousemove still scrolls; this keeps total speed controllable (was ~constant-feeling).
constexpr double kEdgeScrollStepScale = 0.18;
constexpr float kEdgeFillMulInactive = 0.45f;
constexpr float kEdgeFillMulInnerBase = 0.74f;
constexpr float kEdgeFillMulInnerRange = 0.26f;
constexpr int kTrackLabelOverlapPx = 78;
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
#if QT_VERSION >= QT_VERSION_CHECK(5, 15, 0)
    return event->position().toPoint();
#else
    return event->pos();
#endif
}

// 0 at inner boundary of the scroll zone, 1 at the physical screen edge (linear along zone width).
double edgeZoneHeatLinear(int mouseX, int tl, int tr, int xLeftEnd, int xRightStart)
{
    const int leftW = xLeftEnd - tl;
    if (leftW > 0 && mouseX >= tl && mouseX < xLeftEnd) {
        if (leftW <= 1) {
            return 1.0;
        }
        const double span = static_cast<double>(std::max(1, leftW - 1));
        const double t = static_cast<double>(xLeftEnd - 1 - mouseX) / span;
        return std::clamp(t, 0.0, 1.0);
    }
    const int rightW = tr - xRightStart;
    if (rightW > 0 && mouseX > xRightStart && mouseX <= tr) {
        if (rightW <= 1) {
            return 1.0;
        }
        const double span = static_cast<double>(std::max(1, rightW - 1));
        const double t = static_cast<double>(mouseX - xRightStart - 1) / span;
        return std::clamp(t, 0.0, 1.0);
    }
    return 0.0;
}

// Heat only for the left edge band (for painting); pointer past the left uses full heat like edgeScrollStrength.
double edgeZoneHeatLeftOnly(int mouseX, int tl, int xLeftEnd)
{
    if (mouseX < tl) {
        return 1.0;
    }
    const int leftW = xLeftEnd - tl;
    if (leftW > 0 && mouseX >= tl && mouseX < xLeftEnd) {
        if (leftW <= 1) {
            return 1.0;
        }
        const double span = static_cast<double>(std::max(1, leftW - 1));
        const double t = static_cast<double>(xLeftEnd - 1 - mouseX) / span;
        return std::clamp(t, 0.0, 1.0);
    }
    return 0.0;
}

// Heat only for the right edge band.
double edgeZoneHeatRightOnly(int mouseX, int xRightStart, int tr)
{
    if (mouseX > tr) {
        return 1.0;
    }
    const int rightW = tr - xRightStart;
    if (rightW > 0 && mouseX > xRightStart && mouseX <= tr) {
        if (rightW <= 1) {
            return 1.0;
        }
        const double span = static_cast<double>(std::max(1, rightW - 1));
        const double t = static_cast<double>(mouseX - xRightStart - 1) / span;
        return std::clamp(t, 0.0, 1.0);
    }
    return 0.0;
}
}

TimelineEditor::TimelineEditor(QWidget *parent)
    : QWidget(parent)
{
    setAcceptDrops(true);
    setMouseTracking(true);
    setFocusPolicy(Qt::StrongFocus);
    tracks.push_back({TrackType::Video, "V1"});
    tracks.push_back({TrackType::Audio, "A1"});
    setMinimumHeight(260);
    refreshTimelineRangeSignals();
}

TimelineEditor::~TimelineEditor() = default;

void TimelineEditor::setPlaybackMediaPool(std::shared_ptr<burstcon::ConMediaPool> pool)
{
    playbackMediaPool = std::move(pool);
}

void TimelineEditor::setStripMediaPool(std::shared_ptr<burstcon::ConMediaPool> pool)
{
    stripMediaPool = std::move(pool);
}

void TimelineEditor::invalidateSourcePath(const QString &path)
{
    if (path.isEmpty()) {
        return;
    }
    const QString thumbPrefix = path + QStringLiteral("::");
    const QString peakPrefix = path + QStringLiteral("|");
    for (auto it = previewCache.begin(); it != previewCache.end();) {
        if (it.key().startsWith(thumbPrefix)) {
            const QString k = it.key();
            const auto lit = previewCacheLruPos.find(k);
            if (lit != previewCacheLruPos.end()) {
                previewCacheLru.erase(lit.value());
                previewCacheLruPos.erase(lit);
            }
            it = previewCache.erase(it);
        } else {
            ++it;
        }
    }
    for (auto it = peaksStereoCache.begin(); it != peaksStereoCache.end();) {
        if (it.key().startsWith(peakPrefix)) {
            it = peaksStereoCache.erase(it);
        } else {
            ++it;
        }
    }
    const QList<QString> pendPrev = pendingPreviewKeys.values();
    for (const QString &k : pendPrev) {
        if (k.startsWith(thumbPrefix)) {
            pendingPreviewKeys.remove(k);
        }
    }
    const QList<QString> pendPeaks = pendingPeaksKeys.values();
    for (const QString &k : pendPeaks) {
        if (k.startsWith(peakPrefix)) {
            pendingPeaksKeys.remove(k);
        }
    }
    update();
}

void TimelineEditor::purgePresentationCaches()
{
    previewCache.clear();
    previewCacheLru.clear();
    previewCacheLruPos.clear();
    pendingPreviewKeys.clear();
    pendingPeaksKeys.clear();
    peaksStereoCache.clear();
    cacheVideoSlidingByClip.clear();
    cacheVideoHeadByClip.clear();
    timelineVideoSlidingSegments.clear();
    timelineVideoHeadSegments.clear();
    cacheAudioByClip.clear();
    update();
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
    const int nearIdx = markerIndexNearTimeMs(cursorPositionMs);
    if (nearIdx >= 0) {
        markersMs.erase(markersMs.begin() + nearIdx);
        refreshTimelineRangeSignals();
        emit timelineContentChanged();
        update();
        return;
    }
    markersMs.push_back(cursorPositionMs);
    std::sort(markersMs.begin(), markersMs.end());
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::addVideoTrack()
{
    insertTrackAt(static_cast<int>(tracks.size()), TrackType::Video);
    contentGeometryUpdated();
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
}

void TimelineEditor::addAudioTrack()
{
    insertTrackAt(static_cast<int>(tracks.size()), TrackType::Audio);
    contentGeometryUpdated();
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
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

    if (hasRegionSelection && regionEndMs > regionStartMs) {
        const qint64 rs = regionStartMs;
        const qint64 re = regionEndMs;
        for (const auto &clip : clips) {
            const qint64 cEnd = clip.startMs + clip.durationMs;
            if (cEnd <= rs || clip.startMs >= re) {
                continue;
            }
            const qint64 t0 = std::max(rs, clip.startMs);
            const qint64 t1 = std::min(re, cEnd);
            const qint64 dur = t1 - t0;
            if (dur < kMinClipDurationMs) {
                continue;
            }
            ClipEvent c = clip;
            c.startMs = t0 - rs;
            c.durationMs = dur;
            c.sourceInMs = clip.sourceInMs + (t0 - clip.startMs);
            clipboardClips.push_back(c);
        }
        if (!clipboardClips.empty()) {
            return true;
        }
    }

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
    QHash<int, int> oldGroupToNew;
    int firstNewId = -1;
    qint64 pasteMaxEndMs = cursorPositionMs;
    for (const auto &src : clipboardClips) {
        ClipEvent c = src;
        c.id = nextClipId++;
        c.startMs = cursorPositionMs + (src.startMs - sourceMinStart);
        if (!isTrackIndexValid(c.trackIndex)) {
            c.trackIndex = ensureDropTrackForType(c.mediaType, contentBottom() + 20);
        }
        if (src.linkedGroupId >= 0) {
            if (!oldGroupToNew.contains(src.linkedGroupId)) {
                oldGroupToNew.insert(src.linkedGroupId, nextLinkedGroupId++);
            }
            c.linkedGroupId = oldGroupToNew.value(src.linkedGroupId);
        } else {
            c.linkedGroupId = -1;
        }
        clips.push_back(c);
        pasteMaxEndMs = std::max(pasteMaxEndMs, c.startMs + c.durationMs);
        if (firstNewId < 0) {
            firstNewId = c.id;
        }
    }
    selectedClipId = firstNewId;
    setCursorMs(pasteMaxEndMs, true);
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

namespace {
bool clipIntersectsOpenRange(qint64 c0, qint64 c1, qint64 rs, qint64 re)
{
    return c1 > rs && c0 < re;
}
}

bool TimelineEditor::deleteClipsIntersectingRegion()
{
    if (!hasRegionSelection || regionEndMs <= regionStartMs) {
        return false;
    }
    const qint64 rs = regionStartMs;
    const qint64 re = regionEndMs;

    std::unordered_set<int> removeIds;
    for (const auto &c : clips) {
        if (clipIntersectsOpenRange(c.startMs, c.startMs + c.durationMs, rs, re)) {
            for (int id : linkedClipIds(c.id)) {
                removeIds.insert(id);
            }
        }
    }
    if (removeIds.empty()) {
        return false;
    }

    const bool clearedSelection = removeIds.count(selectedClipId) > 0;
    clips.erase(std::remove_if(clips.begin(), clips.end(), [&](const ClipEvent &clip) {
                     return removeIds.count(clip.id) > 0;
                 }),
                 clips.end());
    if (clearedSelection) {
        selectedClipId = -1;
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
    return true;
}

bool TimelineEditor::splitDeleteRegionInterior()
{
    if (!hasRegionSelection || regionEndMs <= regionStartMs) {
        return false;
    }
    const qint64 rs = regionStartMs;
    const qint64 re = regionEndMs;

    std::vector<ClipEvent> additions;
    std::unordered_set<int> removeIds;

    for (const auto &orig : clips) {
        const qint64 c0 = orig.startMs;
        const qint64 c1 = c0 + orig.durationMs;
        if (c1 <= rs || c0 >= re) {
            continue;
        }

        removeIds.insert(orig.id);

        if (c0 >= rs && c1 <= re) {
            continue;
        }

        if (c0 < rs && c1 > rs) {
            const qint64 leftDur = rs - c0;
            if (leftDur >= kMinClipDurationMs) {
                ClipEvent L = orig;
                L.durationMs = leftDur;
                L.sourceInMs = orig.sourceInMs;
                additions.push_back(L);
            }
        }
        if (c1 > re && re > c0) {
            const qint64 rightDur = c1 - re;
            if (rightDur >= kMinClipDurationMs) {
                ClipEvent R = orig;
                R.startMs = re;
                R.durationMs = rightDur;
                R.sourceInMs = orig.sourceInMs + (re - c0);
                additions.push_back(R);
            }
        }
    }

    if (removeIds.empty()) {
        return false;
    }

    const bool clearedSelection = removeIds.count(selectedClipId) > 0;
    clips.erase(std::remove_if(clips.begin(), clips.end(), [&](const ClipEvent &clip) {
                     return removeIds.count(clip.id) > 0;
                 }),
                 clips.end());

    for (auto &c : additions) {
        c.id = nextClipId++;
        c.linkedGroupId = -1;
    }

    for (auto &c : additions) {
        clips.push_back(c);
    }

    if (clearedSelection) {
        selectedClipId = -1;
    }

    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
    return true;
}

std::unordered_set<int> TimelineEditor::clipIdsIntersectingRegion(bool allTracks, int trackIndex) const
{
    std::unordered_set<int> out;
    if (!hasRegionSelection || regionEndMs <= regionStartMs) {
        return out;
    }
    const qint64 rs = regionStartMs;
    const qint64 re = regionEndMs;
    for (const auto &c : clips) {
        if (!clipIntersectsOpenRange(c.startMs, c.startMs + c.durationMs, rs, re)) {
            continue;
        }
        if (!allTracks && c.trackIndex != trackIndex) {
            continue;
        }
        out.insert(c.id);
    }
    return out;
}

bool TimelineEditor::applyNewSourcePathToClip(TimelineEditor::ClipEvent &clip, const QString &path) const
{
    if (path.isEmpty()) {
        return false;
    }
    const qint64 newSourceDuration = defaultDurationForPath(path);
    clip.sourcePath = path;
    clip.sourceDurationMs = newSourceDuration;
    clip.color = colorFromPath(path + QString(clip.mediaType == TrackType::Video ? ":v" : ":a"));
    const qint64 maxIn = std::max<qint64>(0, newSourceDuration - kMinClipDurationMs);
    clip.sourceInMs = std::clamp(clip.sourceInMs, 0LL, maxIn);
    const qint64 maxClipDur = std::max<qint64>(kMinClipDurationMs, newSourceDuration - clip.sourceInMs);
    clip.durationMs = std::min(clip.durationMs, maxClipDur);
    return true;
}

void TimelineEditor::replaceSourceForLinkedClipGroup(int clipId, const QString &path)
{
    if (path.isEmpty()) {
        return;
    }
    for (int id : linkedClipIds(clipId)) {
        ClipEvent *c = findClipById(id);
        if (c) {
            applyNewSourcePathToClip(*c, path);
        }
    }
}

void TimelineEditor::setSourceStartMsForLinkedClipGroup(int clipId, qint64 sourceInMs)
{
    for (int id : linkedClipIds(clipId)) {
        ClipEvent *c = findClipById(id);
        if (!c) {
            continue;
        }
        const qint64 maxIn = std::max<qint64>(0, c->sourceDurationMs - c->durationMs);
        c->sourceInMs = std::clamp(sourceInMs, 0LL, maxIn);
    }
}

void TimelineEditor::replaceSourceForRegionClips(const QString &path, bool allTracks, int trackIndex)
{
    if (path.isEmpty()) {
        return;
    }
    const auto ids = clipIdsIntersectingRegion(allTracks, trackIndex);
    if (ids.empty()) {
        return;
    }
    for (int id : ids) {
        ClipEvent *c = findClipById(id);
        if (c) {
            applyNewSourcePathToClip(*c, path);
        }
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::shiftSourceStartForRegionClips(qint64 deltaMs, bool allTracks, int trackIndex)
{
    const auto ids = clipIdsIntersectingRegion(allTracks, trackIndex);
    if (ids.empty()) {
        return;
    }
    for (int id : ids) {
        ClipEvent *c = findClipById(id);
        if (!c) {
            continue;
        }
        const qint64 maxIn = std::max<qint64>(0, c->sourceDurationMs - c->durationMs);
        const qint64 next = c->sourceInMs + deltaMs;
        c->sourceInMs = std::clamp(next, 0LL, maxIn);
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
}

QString TimelineEditor::firstLocalPathFromMime(const QMimeData *mime)
{
    if (!mime) {
        return {};
    }
    if (mime->hasUrls()) {
        for (const QUrl &url : mime->urls()) {
            if (url.isLocalFile()) {
                const QString p = url.toLocalFile();
                if (!p.isEmpty()) {
                    return p;
                }
            }
        }
    }


    if (mime->hasText()) {
        QString t = mime->text().trimmed();
        if (t.startsWith(QStringLiteral("file:"), Qt::CaseInsensitive)) {
            const QUrl u = QUrl::fromUserInput(t);
            if (u.isLocalFile()) {
                const QString p = u.toLocalFile();
                if (!p.isEmpty() && QFileInfo::exists(p)) {
                    return p;
                }
            }
        }
        if (!t.isEmpty() && QFileInfo::exists(t)) {
            return t;
        }
    }
    return {};
}

void TimelineEditor::clearPendingMorpherPlacement()
{
    if (!pendingMorpherPlacement.active) {
        return;
    }
    pendingMorpherPlacement = {};
    unsetCursor();
    emitOverlayInfo();
    update();
}

QVector<int> TimelineEditor::collectMorpherTargetClipIds()
{
    QVector<int> out;
    QSet<int> seen;

    const auto addClipId = [&](int clipId) {
        if (clipId <= 0 || seen.contains(clipId)) {
            return;
        }
        ClipEvent *c = findClipById(clipId);
        if (!c || !isTrackIndexValid(c->trackIndex)) {
            return;
        }
        if (tracks[static_cast<size_t>(c->trackIndex)].type == TrackType::Morpher) {
            return;
        }
        seen.insert(clipId);
        out.push_back(clipId);
    };

    if (hasRegionSelection && regionEndMs > regionStartMs) {
        const std::unordered_set<int> inRegion = clipIdsIntersectingRegion(true, 0);
        std::unordered_set<int> expanded;
        for (int id : inRegion) {
            expanded.insert(id);
            for (int lid : linkedClipIds(id)) {
                expanded.insert(lid);
            }
        }
        for (int id : expanded) {
            addClipId(id);
        }
    }

    if (out.isEmpty() && selectedClipId >= 0) {
        for (int id : linkedClipIds(selectedClipId)) {
            addClipId(id);
        }
    }

    return out;
}

void TimelineEditor::commitPendingMorpherPlacement(int trackIdx, qint64 startMs)
{
    if (!pendingMorpherPlacement.active) {
        return;
    }
    if (trackIdx < 0 || trackIdx >= static_cast<int>(tracks.size()) ||
        tracks[static_cast<size_t>(trackIdx)].type != TrackType::Morpher) {
        return;
    }
    MorpherEvent m;
    m.id = nextMorpherEventId++;
    m.trackIndex = trackIdx;
    m.startMs = startMs;
    m.durationMs = 1500;
    m.targetClipIds = pendingMorpherPlacement.targetClipIds;
    morpherEvents.push_back(m);
    const int placedId = morpherEvents.back().id;
    clearPendingMorpherPlacement();
    contentGeometryUpdated();
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
    emit morpherEditWindowRequested(placedId);
}

TimelineEditor::ClipEvent TimelineEditor::effectiveClipForRender(const TimelineEditor::ClipEvent &base) const
{
    ClipEvent out = base;
    for (const auto &m : morpherEvents) {
        if (!m.targetClipIds.contains(base.id)) {
            continue;
        }
        applyMorpherPatchToClipCopy(out, m);
    }
    return out;
}

void TimelineEditor::applyMorpherPatchToClipCopy(TimelineEditor::ClipEvent &clip,
                                                 const TimelineEditor::MorpherEvent &m) const
{
    if (!m.newSourcePath.isEmpty()) {
        applyNewSourcePathToClip(clip, m.newSourcePath);
    }
    if (m.absoluteSourceInMs >= 0) {
        const qint64 maxIn = std::max<qint64>(0, clip.sourceDurationMs - clip.durationMs);
        clip.sourceInMs = std::clamp(m.absoluteSourceInMs, 0LL, maxIn);
    } else if (m.sourceInDeltaMs != 0) {
        const qint64 maxIn = std::max<qint64>(0, clip.sourceDurationMs - clip.durationMs);
        clip.sourceInMs = std::clamp(clip.sourceInMs + m.sourceInDeltaMs, 0LL, maxIn);
    }
}

TimelineEditor::MorpherEvent *TimelineEditor::findMorpherById(int id)
{
    for (auto &m : morpherEvents) {
        if (m.id == id) {
            return &m;
        }
    }
    return nullptr;
}

const TimelineEditor::MorpherEvent *TimelineEditor::findMorpherById(int id) const
{
    for (const auto &m : morpherEvents) {
        if (m.id == id) {
            return &m;
        }
    }
    return nullptr;
}

QRect TimelineEditor::morpherEventRect(const TimelineEditor::MorpherEvent &m) const
{
    if (!isTrackIndexValid(m.trackIndex) || tracks[static_cast<size_t>(m.trackIndex)].type != TrackType::Morpher) {
        return {};
    }
    const int y = trackTop(m.trackIndex) + 4;
    const int h = trackHeight() - 8;
    const int x1 = timeToX(m.startMs);
    const int x2 = timeToX(m.startMs + m.durationMs);
    return QRect(std::min(x1, x2), y, std::max(2, std::abs(x2 - x1)), h);
}

int TimelineEditor::morpherEventIdAt(const QPoint &pos) const
{
    for (auto it = morpherEvents.rbegin(); it != morpherEvents.rend(); ++it) {
        if (morpherEventRect(*it).contains(pos)) {
            return it->id;
        }
    }
    return -1;
}

void TimelineEditor::applyReplaceSourceToClip(int clipId, const QString &path)
{
    if (path.isEmpty()) {
        return;
    }
    replaceSourceForLinkedClipGroup(clipId, path);
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::applyReplaceSourceToRegion(const QString &path, bool allTracks, int trackIndex)
{
    replaceSourceForRegionClips(path, allTracks, trackIndex);
}

void TimelineEditor::applyAbsoluteSourceStartToClip(int clipId, qint64 sourceInMs)
{
    setSourceStartMsForLinkedClipGroup(clipId, sourceInMs);
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::applyRegionSourceShift(qint64 deltaMs, bool allTracks, int trackIndex)
{
    shiftSourceStartForRegionClips(deltaMs, allTracks, trackIndex);
}

int TimelineEditor::selectedMorpherEventId() const
{
    return selectedMorpherId;
}

QString TimelineEditor::morpherReplacePath(int morpherEventId) const
{
    const MorpherEvent *m = findMorpherById(morpherEventId);
    return m ? m->newSourcePath : QString();
}

void TimelineEditor::setMorpherReplacePath(int morpherEventId, const QString &path)
{
    MorpherEvent *m = findMorpherById(morpherEventId);
    if (!m) {
        return;
    }
    m->newSourcePath = path;
    emit timelineContentChanged();
    update();
}

qint64 TimelineEditor::morpherSourceInDeltaMs(int morpherEventId) const
{
    const MorpherEvent *m = findMorpherById(morpherEventId);
    return m ? m->sourceInDeltaMs : 0;
}

void TimelineEditor::setMorpherSourceInDeltaMs(int morpherEventId, qint64 deltaMs)
{
    MorpherEvent *m = findMorpherById(morpherEventId);
    if (!m) {
        return;
    }
    m->sourceInDeltaMs = deltaMs;
    emit timelineContentChanged();
    update();
}

qint64 TimelineEditor::morpherAbsoluteSourceInMs(int morpherEventId) const
{
    const MorpherEvent *m = findMorpherById(morpherEventId);
    return m ? m->absoluteSourceInMs : -1;
}

void TimelineEditor::setMorpherAbsoluteSourceInMs(int morpherEventId, qint64 ms)
{
    MorpherEvent *m = findMorpherById(morpherEventId);
    if (!m) {
        return;
    }
    m->absoluteSourceInMs = ms;
    emit timelineContentChanged();
    update();
}

bool TimelineEditor::hasMorpherEvent(int morpherEventId) const
{
    return findMorpherById(morpherEventId) != nullptr;
}

QString TimelineEditor::morpherTargetsDescription(int morpherEventId) const
{
    const MorpherEvent *m = findMorpherById(morpherEventId);
    if (!m || m->targetClipIds.isEmpty()) {
        return tr("No frozen targets.");
    }
    QStringList parts;
    const int n = m->targetClipIds.size();
    const int show = std::min(4, n);
    for (int i = 0; i < show; ++i) {
        parts.push_back(QStringLiteral("#%1").arg(m->targetClipIds[static_cast<int>(i)]));
    }
    QString tail = parts.join(QStringLiteral(", "));
    if (n > show) {
        tail += QStringLiteral(" …");
    }
    return tr("Frozen at creation: %1 clip(s). Clip ids: %2").arg(n).arg(tail);
}

void TimelineEditor::addMorpherTrack()
{
    insertTrackAt(static_cast<int>(tracks.size()), TrackType::Morpher);
    contentGeometryUpdated();
    refreshTimelineRangeSignals();
    emit timelineContentChanged();
}

void TimelineEditor::beginMorpherPlacementFromSelection()
{
    const QVector<int> targets = collectMorpherTargetClipIds();
    if (targets.isEmpty()) {
        QMessageBox::information(
            this,
            tr("Morpher"),
            tr("Nothing to morph: mark a time region over clips, or select a clip on the timeline, then try again."));
        return;
    }
    bool hasMorpherLane = false;
    for (const auto &lane : tracks) {
        if (lane.type == TrackType::Morpher) {
            hasMorpherLane = true;
            break;
        }
    }
    if (!hasMorpherLane) {
        addMorpherTrack();
    }
    pendingMorpherPlacement.active = true;
    pendingMorpherPlacement.targetClipIds = targets;
    setCursor(Qt::PointingHandCursor);
    setFocus(Qt::OtherFocusReason);
    emitOverlayInfo();
    update();
    QMessageBox::information(
        this,
        tr("Place morpher"),
        tr("Locked %n clip target(s).\n\n"
           "Click the purple Morph track at the time where the block should start (you can also drop a file onto that track).\n"
           "Esc cancels.",
           "",
           targets.size()));
}

void TimelineEditor::bakeMorpherEvent(int morpherEventId)
{
    MorpherEvent *me = findMorpherById(morpherEventId);
    if (!me) {
        return;
    }
    for (int clipId : me->targetClipIds) {
        ClipEvent *c = findClipById(clipId);
        if (!c) {
            continue;
        }
        applyMorpherPatchToClipCopy(*c, *me);
    }
    morpherEvents.erase(std::remove_if(morpherEvents.begin(), morpherEvents.end(),
                                       [morpherEventId](const MorpherEvent &x) { return x.id == morpherEventId; }),
                        morpherEvents.end());
    if (selectedMorpherId == morpherEventId) {
        selectedMorpherId = -1;
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::deleteMorpherEvent(int morpherEventId)
{
    morpherEvents.erase(std::remove_if(morpherEvents.begin(), morpherEvents.end(),
                                        [morpherEventId](const MorpherEvent &x) { return x.id == morpherEventId; }),
                         morpherEvents.end());
    if (selectedMorpherId == morpherEventId) {
        selectedMorpherId = -1;
    }
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    update();
}

void TimelineEditor::deleteCurrentSelection()
{
    if (selectedMorpherId >= 0) {
        deleteMorpherEvent(selectedMorpherId);
        return;
    }
    deleteSelectedClip();
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
        const ClipEvent eff = effectiveClipForRender(*it);
        return ActiveClip{eff.sourcePath, eff.startMs, eff.durationMs, eff.sourceInMs};
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
        const ClipEvent eff = effectiveClipForRender(clip);
        result.push_back(ActiveClip{eff.sourcePath, eff.startMs, eff.durationMs, eff.sourceInMs});
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
        const ClipEvent eff = effectiveClipForRender(clip);
        result.push_back(ActiveClip{eff.sourcePath, eff.startMs, eff.durationMs, eff.sourceInMs});
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
        const ClipEvent eff = effectiveClipForRender(clip);
        RenderClip c;
        c.sourcePath = eff.sourcePath;
        c.startMs = eff.startMs;
        c.durationMs = eff.durationMs;
        c.sourceInMs = eff.sourceInMs;
        c.trackIndex = eff.trackIndex;
        c.isVideo = (eff.mediaType == TrackType::Video);
        c.isAudio = (eff.mediaType == TrackType::Audio);
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

bool TimelineEditor::isEditingTimelineContent() const
{
    return dragMode == DragMode::MoveClip ||
        dragMode == DragMode::TrimStart ||
        dragMode == DragMode::TrimEnd;
}

void TimelineEditor::setCacheCoverage(const QHash<QString, QVector<QPair<double, double>>> &clipVideoSliding,
                                      const QHash<QString, QVector<QPair<double, double>>> &clipVideoHead,
                                      const QVector<QPair<double, double>> &timelineVideoSliding,
                                      const QVector<QPair<double, double>> &timelineVideoHead,
                                      const QHash<QString, QVector<QPair<double, double>>> &clipAudioSegments)
{
    cacheVideoSlidingByClip = clipVideoSliding;
    cacheVideoHeadByClip = clipVideoHead;
    timelineVideoSlidingSegments = timelineVideoSliding;
    timelineVideoHeadSegments = timelineVideoHead;
    cacheAudioByClip = clipAudioSegments;
    update();
}

QJsonObject TimelineEditor::toJson() const
{
    QJsonObject root;
    root.insert("version", 3);
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
        QString tstr = "video";
        if (track.type == TrackType::Audio) {
            tstr = "audio";
        } else if (track.type == TrackType::Morpher) {
            tstr = "morpher";
        }
        trackObject.insert("type", tstr);
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

    QJsonArray morphArray;
    for (const auto &m : morpherEvents) {
        QJsonObject mo;
        mo.insert("id", m.id);
        mo.insert("trackIndex", m.trackIndex);
        mo.insert("startMs", static_cast<double>(m.startMs));
        mo.insert("durationMs", static_cast<double>(m.durationMs));
        mo.insert("newSourcePath", m.newSourcePath);
        mo.insert("sourceInDeltaMs", static_cast<double>(m.sourceInDeltaMs));
        mo.insert("absoluteSourceInMs", static_cast<double>(m.absoluteSourceInMs));
        QJsonArray idArr;
        for (int cid : m.targetClipIds) {
            idArr.append(cid);
        }
        mo.insert("targetClipIds", idArr);
        morphArray.append(mo);
    }
    root.insert("morpherEvents", morphArray);
    root.insert("nextMorpherEventId", nextMorpherEventId);

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

    QHash<quint64, int> legacySnowflakeToClipId;

    std::vector<TrackLane> loadedTracks;
    const QJsonArray tracksArray = root.value("tracks").toArray();
    loadedTracks.reserve(static_cast<size_t>(tracksArray.size()));
    for (const auto &trackValue : tracksArray) {
        const QJsonObject trackObject = trackValue.toObject();
        const QString typeString = trackObject.value("type").toString();
        TrackType type = TrackType::Video;
        if (typeString == "audio") {
            type = TrackType::Audio;
        } else if (typeString == "morpher") {
            type = TrackType::Morpher;
        }
        QString name = trackObject.value("name").toString();
        if (name.isEmpty()) {
            const int suffix = static_cast<int>(loadedTracks.size()) + 1;
            if (type == TrackType::Morpher) {
                name = QStringLiteral("Morph %1").arg(suffix);
            } else {
                name = QString("%1%2").arg(type == TrackType::Video ? "V" : "A").arg(suffix);
            }
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
        {
            const QString sfs = clipObject.value("snowflakeId").toString();
            if (!sfs.isEmpty()) {
                legacySnowflakeToClipId.insert(sfs.toULongLong(), clip.id);
            }
        }
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
        if (clip.trackIndex < static_cast<int>(loadedTracks.size()) &&
            loadedTracks[static_cast<size_t>(clip.trackIndex)].type == TrackType::Morpher) {
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

    std::vector<MorpherEvent> loadedMorphs;
    const QJsonArray morphArray = root.value("morpherEvents").toArray();
    int maxMorphId = 0;
    for (const auto &mv : morphArray) {
        const QJsonObject mo = mv.toObject();
        MorpherEvent m;
        m.id = mo.value("id").toInt();
        m.trackIndex = mo.value("trackIndex").toInt(0);
        m.startMs = static_cast<qint64>(mo.value("startMs").toDouble(0.0));
        m.durationMs = static_cast<qint64>(mo.value("durationMs").toDouble(1500.0));
        m.newSourcePath = mo.value("newSourcePath").toString();
        m.sourceInDeltaMs = static_cast<qint64>(mo.value("sourceInDeltaMs").toDouble(0.0));
        m.absoluteSourceInMs = static_cast<qint64>(mo.value("absoluteSourceInMs").toDouble(-1.0));
        m.targetClipIds.clear();
        const QJsonArray idArr = mo.value("targetClipIds").toArray();
        if (!idArr.isEmpty()) {
            for (const auto &v : idArr) {
                const int cid = v.toInt(-1);
                if (cid > 0) {
                    m.targetClipIds.push_back(cid);
                }
            }
        } else {
            const QJsonArray tarr = mo.value("targetSnowflakes").toArray();
            for (const auto &tv : tarr) {
                quint64 sf = 0;
                if (tv.isString()) {
                    sf = tv.toString().toULongLong();
                } else {
                    sf = static_cast<quint64>(tv.toVariant().toULongLong());
                }
                const int cid = legacySnowflakeToClipId.value(sf, -1);
                if (cid > 0) {
                    m.targetClipIds.push_back(cid);
                }
            }
        }
        if (m.id > 0 && m.durationMs > 0 && m.trackIndex >= 0 &&
            m.trackIndex < static_cast<int>(loadedTracks.size()) &&
            loadedTracks[static_cast<size_t>(m.trackIndex)].type == TrackType::Morpher) {
            loadedMorphs.push_back(m);
            maxMorphId = std::max(maxMorphId, m.id);
        }
    }

    tracks = std::move(loadedTracks);
    clips = std::move(loadedClips);
    morpherEvents = std::move(loadedMorphs);
    markersMs = std::move(loadedMarkers);
    clipboardClips.clear();
    previewCache.clear();
    previewCacheLru.clear();
    previewCacheLruPos.clear();
    pendingPreviewKeys.clear();
    pendingPeaksKeys.clear();
    peaksStereoCache.clear();
    sourceDurationProbeCache.clear();
    cacheVideoSlidingByClip.clear();
    cacheVideoHeadByClip.clear();
    timelineVideoSlidingSegments.clear();
    timelineVideoHeadSegments.clear();
    cacheAudioByClip.clear();

    nextClipId = std::max(root.value("nextClipId").toInt(maxClipId + 1), maxClipId + 1);
    nextLinkedGroupId = std::max(root.value("nextLinkedGroupId").toInt(maxLinkedGroupId + 1), maxLinkedGroupId + 1);
    nextMorpherEventId = std::max(root.value("nextMorpherEventId").toInt(maxMorphId + 1), maxMorphId + 1);
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

    contentGeometryUpdated();
    emit loopEnabledChanged(loopRegionEnabled);
    emitRegionChanged();
    refreshTimelineRangeSignals();
    emitOverlayInfo();
    emit timelineContentChanged();
    emit cursorChanged(cursorPositionMs);
    return true;
}

void TimelineEditor::paintEvent(QPaintEvent *event)
{
    Q_UNUSED(event);

    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.fillRect(rect(), QColor(18, 18, 18));

    const int paintBottom = verticalScrollPx + height() - 4;

    p.save();
    p.setClipRect(rect());
    p.translate(0, -verticalScrollPx);
    const QRect rulerRect(timelineLeft(), rulerTop(), timelineRight() - timelineLeft(), rulerHeight());
    p.fillRect(rulerRect, QColor(30, 30, 30));
    p.fillRect(QRect(0, rulerTop(), headerWidth(), rulerHeight()), QColor(26, 26, 26));

    for (int i = 0; i < static_cast<int>(tracks.size()); ++i) {
        const QRect laneRect(timelineLeft(), trackTop(i), timelineRight() - timelineLeft(), trackHeight());
        QColor laneColor(48, 38, 38);
        if (tracks[static_cast<size_t>(i)].type == TrackType::Video) {
            laneColor = QColor(36, 46, 58);
        } else if (tracks[static_cast<size_t>(i)].type == TrackType::Morpher) {
            laneColor = QColor(42, 32, 58);
        }
        p.fillRect(laneRect, laneColor);
        p.fillRect(QRect(0, trackTop(i), headerWidth(), trackHeight()), QColor(24, 24, 24));
    }

    if (pendingMorpherPlacement.active) {
        for (int i = 0; i < static_cast<int>(tracks.size()); ++i) {
            if (tracks[static_cast<size_t>(i)].type != TrackType::Morpher) {
                continue;
            }
            const QRect laneRect(timelineLeft(), trackTop(i), timelineRight() - timelineLeft(), trackHeight());
            p.fillRect(laneRect, QColor(96, 200, 140, 52));
            p.setPen(QPen(QColor(140, 235, 170, 200), 2));
            p.drawRoundedRect(laneRect.adjusted(2, 2, -2, -2), 5, 5);
        }
    }

    const qint64 visible = contentVisibleMs();
    const qint64 viewEndMs = timelineViewStartMs + visible;
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

    // Video cache strip (above ruler): draw after tick grid so vertical lines do not erase it.
    {
        const int tlW = timelineRight() - timelineLeft();
        const int cacheH = 4;
        const QRect timelineCacheStrip(timelineLeft(), std::max(2, rulerTop() - cacheH - 1), tlW, cacheH);
        auto paintSegRow = [&](const QVector<QPair<double, double>> &segs, const QRect &row, const QColor &col) {
            for (const auto &segment : segs) {
                const double a = std::clamp(segment.first, 0.0, 1.0);
                const double b = std::clamp(segment.second, 0.0, 1.0);
                if (b <= a) {
                    continue;
                }
                const int x1 = row.left() + static_cast<int>(a * row.width());
                const int x2 = row.left() + static_cast<int>(b * row.width());
                p.fillRect(QRect(x1, row.top(), std::max(1, x2 - x1), row.height()), col);
            }
        };
        // Green = decoded frame in ConCache (LRU). Yellow overlay = also in region-head window.
        paintSegRow(timelineVideoSlidingSegments, timelineCacheStrip, QColor(72, 168, 96));
        paintSegRow(timelineVideoHeadSegments, timelineCacheStrip, QColor(255, 210, 72, 150));
    }

    if (hasRegionSelection && regionEndMs > regionStartMs) {
        const int x1 = timeToX(regionStartMs);
        const int x2 = timeToX(regionEndMs);
        const int left = std::min(x1, x2);
        const int right = std::max(x1, x2);
        const int regionW = std::max(1, right - left);
        const int top = rulerTop();
        const int h = paintBottom - top;

        constexpr int kRegionFadeMaxPx = 56;
        constexpr int kRegionAlphaEdge = 58;
        constexpr int kRegionAlphaCenter = 14;
        const QColor baseBlue(84, 124, 255);

        const int fadeW = std::min(kRegionFadeMaxPx, std::max(1, regionW / 2));
        const int midLeft = left + fadeW;
        const int midRight = right - fadeW;

        if (regionW == 1) {
            p.fillRect(QRect(left, top, 1, h),
                       QColor(baseBlue.red(), baseBlue.green(), baseBlue.blue(), kRegionAlphaEdge));
        } else {
            if (midLeft < midRight) {
                p.fillRect(QRect(midLeft, top, midRight - midLeft, h),
                           QColor(baseBlue.red(), baseBlue.green(), baseBlue.blue(), kRegionAlphaCenter));
            }

            QLinearGradient gradL(QPointF(left, top), QPointF(left + fadeW, top));
            gradL.setColorAt(0.0, QColor(baseBlue.red(), baseBlue.green(), baseBlue.blue(), kRegionAlphaEdge));
            gradL.setColorAt(1.0, QColor(baseBlue.red(), baseBlue.green(), baseBlue.blue(), kRegionAlphaCenter));
            p.fillRect(QRect(left, top, fadeW, h), QBrush(gradL));

            QLinearGradient gradR(QPointF(right, top), QPointF(right - fadeW, top));
            gradR.setColorAt(0.0, QColor(baseBlue.red(), baseBlue.green(), baseBlue.blue(), kRegionAlphaEdge));
            gradR.setColorAt(1.0, QColor(baseBlue.red(), baseBlue.green(), baseBlue.blue(), kRegionAlphaCenter));
            p.fillRect(QRect(right - fadeW, top, fadeW, h), QBrush(gradR));
        }

        p.setPen(QPen(QColor(102, 152, 255), 2));
        p.drawLine(x1, rulerTop(), x1, paintBottom);
        p.drawLine(x2, rulerTop(), x2, paintBottom);
    }

    for (const auto &clip : clips) {
        const QRect r = clipRect(clip);
        if (!r.intersects(QRect(timelineLeft(), rulerTop(), timelineRight() - timelineLeft(), contentBottom() - rulerTop()))) {
            continue;
        }
        const ClipEvent drawClip = effectiveClipForRender(clip);
        p.setPen((clip.id == selectedClipId) ? QColor(255, 255, 255) : QColor(25, 25, 25));
        p.setBrush(clip.color);
        p.drawRoundedRect(r, 4, 4);

        if (clip.mediaType == TrackType::Video) {
            const int thumbW = std::max(28, r.height() * 16 / 9);
            const int innerLeft = r.left() + 2;
            const int innerRight = r.right() - 2;
            const int innerWidth = std::max(1, innerRight - innerLeft);
            const qint64 durMs = std::max<qint64>(1, drawClip.durationMs);
            const qint64 stepMs = videoClipThumbnailSourceStepMs(drawClip.durationMs, innerWidth, thumbW);
            for (qint64 srcOff = 0; srcOff < durMs; srcOff += stepMs) {
                const double rel =
                    durMs > 1 ? static_cast<double>(srcOff) / static_cast<double>(durMs - 1) : 0.0;
                const int centerX =
                    innerLeft + static_cast<int>(std::llround(std::clamp(rel, 0.0, 1.0) * static_cast<double>(innerWidth - 1)));
                const int tw = std::min(thumbW, innerRight - innerLeft);
                const int x0 = std::clamp(centerX - tw / 2, innerLeft, std::max(innerLeft, innerRight - tw));
                const QRect thumbRect(x0, r.top() + 2, std::min(tw, innerRight - x0), r.height() - 4);
                const QImage preview = cachedVideoPreview(drawClip.sourcePath, drawClip.sourceInMs + srcOff);
                if (!preview.isNull()) {
                    p.drawImage(thumbRect, preview);
                }
            }
        } else {
            const int usable = std::max(1, r.width() - 6);
            const auto peaksStereo = cachedAudioPeaksStereo(
                drawClip.sourcePath, drawClip.sourceInMs, drawClip.durationMs, drawClip.sourceDurationMs, usable);
            if (!peaksStereo.isEmpty()) {
                const int midY = r.center().y();
                const int halfH = std::max(2, (r.height() / 2) - 2);
                for (int x = 0; x < usable; ++x) {
                    const int pi =
                        (usable <= 1 || peaksStereo.size() <= 1)
                            ? 0
                            : static_cast<int>((static_cast<qint64>(x) * static_cast<qint64>(peaksStereo.size() - 1)) /
                                               std::max<qint64>(1, static_cast<qint64>(usable - 1)));
                    const int idx = std::clamp(pi, 0, peaksStereo.size() - 1);
                    const int ampL = std::clamp(peaksStereo[idx].first, 1, halfH);
                    const int ampR = std::clamp(peaksStereo[idx].second, 1, halfH);
                    p.setPen(QColor(170, 210, 255, 230));
                    p.drawLine(r.left() + 3 + x, midY - ampL, r.left() + 3 + x, midY - 1);
                    p.setPen(QColor(230, 190, 255, 230));
                    p.drawLine(r.left() + 3 + x, midY + 1, r.left() + 3 + x, midY + ampR);
                }
            }
        }
        if (!drawClip.sourcePath.isEmpty() && !QFileInfo::exists(drawClip.sourcePath)) {
            p.fillRect(r, QColor(55, 25, 25, 110));
            p.setPen(QPen(QColor(255, 140, 140), 1, Qt::DashLine));
            p.drawRoundedRect(r.adjusted(1, 1, -2, -2), 3, 3);
        }
        if (shouldDrawClipLabel(clip)) {
            const QString fileName = QFileInfo(drawClip.sourcePath).fileName();
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

        const QString ckey = clipCoverageKey(clip);
        const QRect barRect(r.left() + 3, r.top() - 6, std::max(6, r.width() - 6), 4);
        if (clip.mediaType == TrackType::Video) {
            auto paintClipSegs = [&](const QVector<QPair<double, double>> &segs, const QColor &col) {
                for (const auto &segment : segs) {
                    const double a = std::clamp(segment.first, 0.0, 1.0);
                    const double b = std::clamp(segment.second, 0.0, 1.0);
                    if (b <= a) {
                        continue;
                    }
                    const int x1 = barRect.left() + static_cast<int>(a * barRect.width());
                    const int x2 = barRect.left() + static_cast<int>(b * barRect.width());
                    p.fillRect(QRect(x1, barRect.top(), std::max(1, x2 - x1), barRect.height()), col);
                }
            };
            paintClipSegs(cacheVideoSlidingByClip.value(ckey), QColor(72, 168, 96));
            paintClipSegs(cacheVideoHeadByClip.value(ckey), QColor(255, 210, 72, 150));
        } else if (clip.mediaType == TrackType::Audio) {
            const QColor barColor(114, 214, 255);
            for (const auto &segment : cacheAudioByClip.value(ckey)) {
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
    }

    for (const auto &me : morpherEvents) {
        const QRect r = morpherEventRect(me);
        if (r.isEmpty() || !r.intersects(QRect(timelineLeft(), rulerTop(), timelineRight() - timelineLeft(), contentBottom() - rulerTop()))) {
            continue;
        }
        p.setPen((me.id == selectedMorpherId) ? QColor(255, 255, 255) : QColor(40, 30, 55));
        p.setBrush(QColor(140, 90, 200, 200));
        p.drawRoundedRect(r, 4, 4);
        const QString label = QStringLiteral("Morpher (%1)").arg(me.targetClipIds.size());
        p.setPen(QColor(240, 240, 240));
        p.drawText(r.adjusted(6, 4, -4, -4), Qt::AlignLeft | Qt::AlignTop, label);
    }

    // Edge-scroll gradients on top of clips (hit zone still extends to the timeline edge on the right).
    const int gradTop = verticalScrollPx;
    const int h = height();
    const int tl = timelineLeft();
    const int tr = timelineRight();
    int xLeftEnd = tl;
    int xRightStart = tr;
    edgeScrollZoneBounds(&xLeftEnd, &xRightStart);
    const int leftZoneW = std::max(0, xLeftEnd - tl);
    const int rightX0 = xRightStart;
    // Paint through the real timeline right edge so the gradient matches the scroll hit zone (no dead strip).
    const int rightPaintEnd = tr;
    const int rightZoneW = std::max(0, rightPaintEnd - rightX0);
    const bool pointerInWidget = rect().contains(lastHoverPos);
    const int mx = lastHoverPos.x();
    const double heatL = pointerInWidget ? edgeZoneHeatLeftOnly(mx, tl, xLeftEnd) : 0.0;
    const double heatR = pointerInWidget ? edgeZoneHeatRightOnly(mx, xRightStart, tr) : 0.0;
    const float fillMulL = (heatL > 0.0)
        ? std::min(1.0f, kEdgeFillMulInnerBase + kEdgeFillMulInnerRange * static_cast<float>(heatL))
        : kEdgeFillMulInactive;
    const float fillMulR = (heatR > 0.0)
        ? std::min(1.0f, kEdgeFillMulInnerBase + kEdgeFillMulInnerRange * static_cast<float>(heatR))
        : kEdgeFillMulInactive;
    const int stripeAlphaL =
        (heatL > 0.0) ? std::clamp(58 + static_cast<int>(128.0 * heatL), 58, 210) : 36;
    const int stripeAlphaR =
        (heatR > 0.0) ? std::clamp(58 + static_cast<int>(128.0 * heatR), 58, 210) : 36;

    const auto paintEdgeZone = [&](int x, int w, bool fadeFromLeft, float fillMul, int stripeAlpha, bool hotThisSide) {
        if (w <= 0) {
            return;
        }
        QLinearGradient g;
        if (fadeFromLeft) {
            g.setStart(x, gradTop);
            g.setFinalStop(x + w, gradTop);
            g.setColorAt(0.0, QColor(255, 182, 104, static_cast<int>(56 * fillMul)));
            g.setColorAt(0.25, QColor(255, 178, 100, static_cast<int>(38 * fillMul)));
            g.setColorAt(0.55, QColor(255, 178, 100, static_cast<int>(22 * fillMul)));
            g.setColorAt(0.82, QColor(255, 178, 100, static_cast<int>(10 * fillMul)));
            g.setColorAt(1.0, QColor(255, 178, 100, 0));
        } else {
            g.setStart(x + w, gradTop);
            g.setFinalStop(x, gradTop);
            g.setColorAt(0.0, QColor(255, 182, 104, static_cast<int>(56 * fillMul)));
            g.setColorAt(0.25, QColor(255, 178, 100, static_cast<int>(38 * fillMul)));
            g.setColorAt(0.55, QColor(255, 178, 100, static_cast<int>(22 * fillMul)));
            g.setColorAt(0.82, QColor(255, 178, 100, static_cast<int>(10 * fillMul)));
            g.setColorAt(1.0, QColor(255, 178, 100, 0));
        }
        p.fillRect(QRect(x, gradTop, w, h), QBrush(g));
        const int hairAlpha = hotThisSide ? std::min(255, 28 + stripeAlpha / 2) : 18;
        p.setPen(QPen(QColor(255, 200, 128, hairAlpha), 1));
        p.drawLine(x, gradTop, x + w - 1, gradTop);
        p.setPen(QPen(QColor(255, 188, 110, stripeAlpha), 1));
        p.drawLine(x, gradTop + 1, x + w - 1, gradTop + 1);
    };

    paintEdgeZone(tl, leftZoneW, true, fillMulL, stripeAlphaL, heatL > 0.0);
    paintEdgeZone(rightX0, rightZoneW, false, fillMulR, stripeAlphaR, heatR > 0.0);

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

    const int cursorX = timeToX(cursorPositionMs);
    p.setPen(QPen(QColor(255, 60, 60, 120), 6));
    p.drawLine(cursorX, rulerTop(), cursorX, paintBottom);
    p.setPen(QPen(QColor(255, 100, 100), 2));
    p.drawLine(cursorX, rulerTop(), cursorX, paintBottom);

    for (int i = 0; i < static_cast<int>(tracks.size()); ++i) {
        const int top = trackTop(i);
        const int labelRight = std::min(timelineLeft() + kTrackLabelOverlapPx, timelineRight() - 2);
        QLinearGradient labelBg(0, 0, labelRight, 0);
        labelBg.setColorAt(0.0, QColor(18, 18, 18, 252));
        labelBg.setColorAt(0.45, QColor(18, 18, 18, 160));
        labelBg.setColorAt(1.0, QColor(18, 18, 18, 0));
        p.fillRect(QRect(0, top, labelRight, trackHeight()), labelBg);
        const QString &name = tracks[static_cast<size_t>(i)].name;
        p.setPen(QColor(0, 0, 0, 200));
        p.drawText(9, top + 24, name);
        p.setPen(QColor(210, 210, 210));
        p.drawText(8, top + 23, name);
    }

    p.restore();
}

void TimelineEditor::mousePressEvent(QMouseEvent *event)
{
    if (event->button() != Qt::LeftButton) {
        QWidget::mousePressEvent(event);
        return;
    }
    const QPoint pos = mapToTimelineContent(eventPos(event));
    setFocus(Qt::MouseFocusReason);

    if (pendingMorpherPlacement.active) {
        const int trIdx = trackIndexAtY(pos.y());
        if (trIdx >= 0 && tracks[static_cast<size_t>(trIdx)].type == TrackType::Morpher) {
            commitPendingMorpherPlacement(trIdx, snapTimeMs(xToTimeMs(pos.x())));
            event->accept();
            return;
        }
        const bool inTimelineX = pos.x() >= timelineLeft() && pos.x() <= timelineRight();
        if (inTimelineX && pos.y() >= rulerTop() && pos.y() <= (rulerTop() + rulerHeight())) {
            const qint64 pointerMs = snapTimeMs(xToTimeMs(pos.x()));
            setCursorMs(pointerMs, false);
            selectedClipId = -1;
            selectedMorpherId = -1;
            update();
            event->accept();
            return;
        }
        event->accept();
        return;
    }

    const int hitMarker = markerIndexAt(pos);
    if (hitMarker >= 0) {
        selectedClipId = -1;
        selectedMorpherId = -1;
        dragMode = DragMode::MoveMarker;
        dragMarkerIndex = hitMarker;
        dragClipId = -1;
        dragChangedTimelineContent = false;
        setCursorMs(snapTimeMs(markersMs[static_cast<size_t>(hitMarker)]), false);
        update();
        return;
    }

    const int morpherHit = morpherEventIdAt(pos);
    if (morpherHit >= 0) {
        selectedMorpherId = morpherHit;
        selectedClipId = -1;
        dragMode = DragMode::None;
        dragClipId = -1;
        dragMarkerIndex = -1;
        const qint64 pointerMs = xToTimeMs(pos.x());
        setCursorMs(snapTimeMs(pointerMs));
        update();
        return;
    }
    selectedMorpherId = -1;

    const bool inTimelineX = pos.x() >= timelineLeft() && pos.x() <= timelineRight();
    selectedClipId = clipIdAt(pos);
    const qint64 pointerMs = xToTimeMs(pos.x());
    setCursorMs(snapTimeMs(pointerMs));

    dragMode = DragMode::None;
    dragMarkerIndex = -1;
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
    } else if (inTimelineX && pos.y() >= (rulerTop() + rulerHeight()) &&
               pos.y() <= (verticalScrollPx + height() - bottomPadding())) {
        dragMode = DragMode::RegionSelect;
        regionAnchorMs = snapTimeMsForRegion(pointerMs);
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
    if (pendingMorpherPlacement.active) {
        event->accept();
        return;
    }
    const QPoint pos = mapToTimelineContent(eventPos(event));
    const int morpherHit = morpherEventIdAt(pos);
    if (morpherHit >= 0) {
        selectedMorpherId = morpherHit;
        selectedClipId = -1;
        emit morpherEditWindowRequested(morpherHit);
        update();
        event->accept();
        return;
    }
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
    const QPoint wpos = eventPos(event);
    if (lastHoverPos != wpos) {
        lastHoverPos = wpos;
        int xle = timelineLeft();
        int xrs = timelineRight();
        edgeScrollZoneBounds(&xle, &xrs);
        const int tl = timelineLeft();
        const int tr = timelineRight();
        if ((wpos.x() >= tl && wpos.x() < xle) || (wpos.x() > xrs && wpos.x() <= tr)) {
            update();
        }
    }
    const QPoint pos = mapToTimelineContent(wpos);
    if (!(event->buttons() & Qt::LeftButton) || dragMode == DragMode::None) {
        if (pendingMorpherPlacement.active) {
            const int trIdx = trackIndexAtY(pos.y());
            if (trIdx >= 0 && tracks[static_cast<size_t>(trIdx)].type == TrackType::Morpher) {
                setCursor(Qt::PointingHandCursor);
            } else {
                setCursor(Qt::ArrowCursor);
            }
            QWidget::mouseMoveEvent(event);
            return;
        }
        const int hoverClip = clipIdAt(pos);
        if (hoverClip >= 0) {
            const ClipEvent *clip = findClipById(hoverClip);
            if (clip && (isNearClipStartEdge(*clip, pos.x()) || isNearClipEndEdge(*clip, pos.x()))) {
                setCursor(Qt::SizeHorCursor);
            } else {
                setCursor(Qt::OpenHandCursor);
            }
        } else if (markerIndexAt(pos) >= 0) {
            setCursor(Qt::SizeHorCursor);
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

    const qint64 pointerMsRaw =
        (dragMode == DragMode::MoveMarker) ? xToTimeMs(pos.x()) : adjustedDragPointerMs(pos);
    const qint64 pointerMs = snapTimeMs(pointerMsRaw);
    const qint64 pointerMsRegion = snapTimeMsForRegion(pointerMsRaw);
    ClipEvent *clip = findClipById(dragClipId);
    if (dragMode == DragMode::MoveMarker && dragMarkerIndex >= 0
        && dragMarkerIndex < static_cast<int>(markersMs.size())) {
        const qint64 t = std::max<qint64>(0, snapTimeMs(pointerMsRaw));
        markersMs[static_cast<size_t>(dragMarkerIndex)] = t;
        setCursorMs(t, false);
        refreshTimelineRangeSignals();
    } else if (dragMode == DragMode::ScrubCursor) {
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
        setCursorMs(pointerMsRegion);
        if (hasRegionSelection) {
            setRegionBounds(pointerMsRegion, regionEndMs);
        }
    } else if (dragMode == DragMode::MoveRegionEnd) {
        setCursorMs(pointerMsRegion);
        if (hasRegionSelection) {
            setRegionBounds(regionStartMs, pointerMsRegion);
        }
    } else if (dragMode == DragMode::RegionSelect) {
        setCursorMs(pointerMsRegion);
        setRegionBounds(regionAnchorMs, pointerMsRegion);
    }
    emitOverlayInfo();
    update();
}

void TimelineEditor::mouseReleaseEvent(QMouseEvent *event)
{
    const DragMode finishedMode = dragMode;
    dragMode = DragMode::None;
    dragClipId = -1;
    dragMarkerIndex = -1;
    if (finishedMode == DragMode::MoveMarker) {
        std::sort(markersMs.begin(), markersMs.end());
        refreshTimelineRangeSignals();
        emit timelineContentChanged();
    }
    dragInitialStarts.clear();
    dragInitialDurations.clear();
    dragInitialSourceIns.clear();
    if (dragChangedTimelineContent) {
        dragChangedTimelineContent = false;
        emit timelineContentChanged();
    }
    if (pendingMorpherPlacement.active) {
        setCursor(Qt::PointingHandCursor);
    } else {
        unsetCursor();
    }
    QWidget::mouseReleaseEvent(event);
}

void TimelineEditor::leaveEvent(QEvent *event)
{
    if (lastHoverPos != QPoint(-1, -1)) {
        lastHoverPos = QPoint(-1, -1);
        update();
    }
    QWidget::leaveEvent(event);
}

void TimelineEditor::keyPressEvent(QKeyEvent *event)
{
    if (pendingMorpherPlacement.active && event->key() == Qt::Key_Escape) {
        clearPendingMorpherPlacement();
        event->accept();
        return;
    }
    QWidget::keyPressEvent(event);
}

void TimelineEditor::resizeEvent(QResizeEvent *event)
{
    QWidget::resizeEvent(event);
    clampVerticalScroll();
    emit timelineViewLayoutChanged();
}

void TimelineEditor::contextMenuEvent(QContextMenuEvent *event)
{
    const QPoint menuPos = mapToTimelineContent(event->pos());
    const int markerIdx = markerIndexAt(menuPos);
    if (markerIdx >= 0) {
        QMenu menu(this);
        QAction *deleteMarkerAction = menu.addAction("Delete marker");
        QAction *chosen = menu.exec(event->globalPos());
        if (chosen == deleteMarkerAction) {
            markersMs.erase(markersMs.begin() + markerIdx);
            refreshTimelineRangeSignals();
            emit timelineContentChanged();
            update();
        }
        return;
    }

    const int morpherHit = morpherEventIdAt(menuPos);
    if (morpherHit >= 0) {
        selectedMorpherId = morpherHit;
        selectedClipId = -1;
        QMenu menu(this);
        QAction *bakeAct = menu.addAction(tr("Bake morphing (apply to clips)"));
        QAction *deleteAct = menu.addAction(tr("Delete morpher"));
        QAction *editWin = menu.addAction(tr("Open morpher editor window"));
        QAction *chosen = menu.exec(event->globalPos());
        if (chosen == bakeAct) {
            bakeMorpherEvent(morpherHit);
        } else if (chosen == deleteAct) {
            deleteMorpherEvent(morpherHit);
        } else if (chosen == editWin) {
            emit morpherEditWindowRequested(morpherHit);
        }
        update();
        return;
    }

    const int id = clipIdAt(menuPos);
    const int clickTrack = trackIndexAtY(menuPos.y());
    const bool regionUseful = hasRegionSelection && regionEndMs > regionStartMs;

    auto runRegionShift = [&](bool allTracks) {
        if (!allTracks && clickTrack < 0) {
            QMessageBox::information(this,
                                     tr("Region"),
                                     tr("To use \"This track\", right-click on a clip or on a track row (not the ruler)."));
            return;
        }
        emit sourceOffsetPanelRequestedForRegion(allTracks, clickTrack);
    };

    if (id < 0) {
        if (!regionUseful) {
            QWidget::contextMenuEvent(event);
            return;
        }
        QMenu menu(this);
        QMenu *repSub = menu.addMenu(tr("Region: Replace source"));
        QAction *repThis = repSub->addAction(tr("This track"));
        QAction *repAll = repSub->addAction(tr("All tracks"));
        QMenu *shiftSub = menu.addMenu(tr("Region: Shift source start"));
        QAction *shiftThis = shiftSub->addAction(tr("This track"));
        QAction *shiftAll = shiftSub->addAction(tr("All tracks"));
        menu.addSeparator();
        QAction *morphFromRegion = menu.addAction(tr("Add morpher (clips in region)…"));
        QAction *chosen = menu.exec(event->globalPos());
        if (!chosen) {
            return;
        }
        if (chosen == morphFromRegion) {
            beginMorpherPlacementFromSelection();
            return;
        }
        if (chosen == repThis) {
            if (clickTrack < 0) {
                QMessageBox::information(this,
                                         tr("Region"),
                                         tr("To use \"This track\", right-click on a clip or on a track row (not the ruler)."));
                return;
            }
            emit replaceSourcePanelRequestedForRegion(false, clickTrack);
        } else if (chosen == repAll) {
            emit replaceSourcePanelRequestedForRegion(true, clickTrack);
        } else if (chosen == shiftThis) {
            runRegionShift(false);
        } else if (chosen == shiftAll) {
            runRegionShift(true);
        }
        return;
    }

    selectedMorpherId = -1;
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
    QAction *replaceSourceAction = menu.addAction(tr("Open replace source panel"));
    QAction *sourceStartAction = menu.addAction(tr("Open source offset panel"));
    QAction *morphPlaceAction = menu.addAction(tr("Add morpher (freeze targets, place on Morph track)…"));

    QAction *regRepThis = nullptr;
    QAction *regRepAll = nullptr;
    QAction *regShiftThis = nullptr;
    QAction *regShiftAll = nullptr;
    if (regionUseful) {
        QMenu *repSub = menu.addMenu(tr("Region: Replace source"));
        regRepThis = repSub->addAction(tr("This track"));
        regRepAll = repSub->addAction(tr("All tracks"));
        QMenu *shiftSub = menu.addMenu(tr("Region: Shift source start"));
        regShiftThis = shiftSub->addAction(tr("This track"));
        regShiftAll = shiftSub->addAction(tr("All tracks"));
    }

    QAction *chosen = menu.exec(event->globalPos());
    if (!chosen) {
        return;
    }
    if (chosen == unlinkAction) {
        unlinkClipGroup(id);
    } else if (chosen == copyAction) {
        copySelectedToClipboard();
    } else if (chosen == deleteAction) {
        if (hasRegionSelection && regionEndMs > regionStartMs) {
            deleteClipsIntersectingRegion();
        } else {
            deleteSelectedClip();
        }
    } else if (chosen == magnetizeAction) {
        magnetizeSelectedClip();
    } else if (chosen == splitAction) {
        if (hasRegionSelection && regionEndMs > regionStartMs) {
            splitDeleteRegionInterior();
        } else {
            splitSelectedClipAtCursor();
        }
    } else if (chosen == replaceSourceAction) {
        emit replaceSourcePanelRequestedForClip(id);
    } else if (chosen == sourceStartAction) {
        emit sourceOffsetPanelRequestedForClip(id);
    } else if (chosen == morphPlaceAction) {
        beginMorpherPlacementFromSelection();
    } else if (regRepThis && chosen == regRepThis) {
        if (clickTrack < 0) {
            QMessageBox::information(this,
                                     tr("Region"),
                                     tr("To use \"This track\", right-click on a clip or on a track row (not the ruler)."));
            return;
        }
        emit replaceSourcePanelRequestedForRegion(false, clickTrack);
    } else if (regRepAll && chosen == regRepAll) {
        emit replaceSourcePanelRequestedForRegion(true, clickTrack);
    } else if (regShiftThis && chosen == regShiftThis) {
        runRegionShift(false);
    } else if (regShiftAll && chosen == regShiftAll) {
        runRegionShift(true);
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
    if (pendingMorpherPlacement.active) {
        const QPoint pos = mapToTimelineContent(eventPos(event));
        int trackIdx = trackIndexAtY(pos.y());
        if (trackIdx < 0 || trackIdx >= static_cast<int>(tracks.size()) ||
            tracks[static_cast<size_t>(trackIdx)].type != TrackType::Morpher) {
            const auto r =
                QMessageBox::question(this,
                                      tr("Morpher"),
                                      tr("Drop on a Morph track. No morpher track at this position. Create a Morph track and place the event there?"),
                                      QMessageBox::Yes | QMessageBox::No,
                                      QMessageBox::Yes);
            if (r == QMessageBox::Yes) {
                addMorpherTrack();
                trackIdx = static_cast<int>(tracks.size()) - 1;
            } else {
                event->ignore();
                return;
            }
        }
        commitPendingMorpherPlacement(trackIdx, snapTimeMs(xToTimeMs(pos.x())));
        event->acceptProposedAction();
        return;
    }

    const QPoint pos = mapToTimelineContent(eventPos(event));
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
    return width() - 2;
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

QPoint TimelineEditor::mapToTimelineContent(const QPoint &widgetPos) const
{
    return QPoint(widgetPos.x(), widgetPos.y() + verticalScrollPx);
}

int TimelineEditor::timelineContentHeightPx() const
{
    return contentBottom() + bottomPadding();
}

int TimelineEditor::timelineVerticalScroll() const
{
    return verticalScrollPx;
}

void TimelineEditor::setTimelineVerticalScroll(int px)
{
    const int cap = std::max(0, timelineContentHeightPx() - height());
    const int v = std::clamp(px, 0, cap);
    if (v == verticalScrollPx) {
        return;
    }
    verticalScrollPx = v;
    emit timelineVerticalScrollChanged(v);
    update();
}

void TimelineEditor::clampVerticalScroll()
{
    const int cap = std::max(0, timelineContentHeightPx() - height());
    const int v = std::clamp(verticalScrollPx, 0, cap);
    if (v != verticalScrollPx) {
        verticalScrollPx = v;
        emit timelineVerticalScrollChanged(v);
    }
}

void TimelineEditor::contentGeometryUpdated()
{
    clampVerticalScroll();
    emit timelineViewLayoutChanged();
    update();
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
    for (auto &m : morpherEvents) {
        if (m.trackIndex >= index) {
            ++m.trackIndex;
        }
    }
    int sameTypeCount = 0;
    for (const auto &track : tracks) {
        if (track.type == type) {
            ++sameTypeCount;
        }
    }
    QString laneName;
    if (type == TrackType::Video) {
        laneName = QStringLiteral("V%1").arg(sameTypeCount + 1);
    } else if (type == TrackType::Audio) {
        laneName = QStringLiteral("A%1").arg(sameTypeCount + 1);
    } else {
        laneName = QStringLiteral("Morph %1").arg(sameTypeCount + 1);
    }
    tracks.insert(tracks.begin() + index, TrackLane{type, laneName});
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

QString TimelineEditor::clipSourcePath(int clipId) const
{
    const ClipEvent *c = findClipById(clipId);
    return c ? c->sourcePath : QString();
}

qint64 TimelineEditor::clipSourceInMs(int clipId) const
{
    const ClipEvent *c = findClipById(clipId);
    return c ? c->sourceInMs : 0;
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
    for (const auto &me : morpherEvents) {
        maxMs = std::max(maxMs, me.startMs + me.durationMs + 1000);
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

qint64 TimelineEditor::edgeScrollMarginMs() const
{
    const qint64 visible = contentVisibleMs();
    if (visible <= 1) {
        return 1;
    }
    // ~2/12 of the visible window on each side (click-to-scroll uses this via ensureCursorVisible).
    qint64 m = std::max<qint64>(visible * 2 / 12, 120);
    if (m * 2 >= visible) {
        return std::max<qint64>(1, visible / 2 - 1);
    }
    return m;
}

void TimelineEditor::edgeScrollZoneBounds(int *xLeftEnd, int *xRightStart) const
{
    const int tl = timelineLeft();
    const int tr = timelineRight();
    const qint64 visible = contentVisibleMs();
    const qint64 margin = edgeScrollMarginMs();
    if (xLeftEnd) {
        *xLeftEnd = std::clamp(timeToX(timelineViewStartMs + margin), tl, tr);
    }
    if (xRightStart) {
        *xRightStart = std::clamp(timeToX(timelineViewStartMs + visible - margin), tl, tr);
    }
}

double TimelineEditor::edgeScrollStrength(int mouseX, int tl, int tr, int xLeftEnd, int xRightStart) const
{
    // Pointer left of the timeline (still in "scroll left" band) must be full heat, not 0 — otherwise speed sticks at minimum.
    double t = edgeZoneHeatLinear(mouseX, tl, tr, xLeftEnd, xRightStart);
    if (mouseX < tl) {
        t = 1.0;
    } else if (mouseX > tr) {
        t = 1.0;
    }
    if (t <= 0.0) {
        return 0.0;
    }
    return std::pow(t, kEdgeScrollSpeedGamma);
}

int TimelineEditor::markerIndexAt(const QPoint &pos) const
{
    const int rulerBottom = rulerTop() + rulerHeight();
    if (pos.y() < rulerTop() || pos.y() > rulerBottom) {
        return -1;
    }
    if (pos.x() < timelineLeft() || pos.x() > timelineRight()) {
        return -1;
    }
    for (int i = 0; i < static_cast<int>(markersMs.size()); ++i) {
        const int mx = timeToX(markersMs[static_cast<size_t>(i)]);
        if (std::abs(pos.x() - mx) <= 7) {
            return i;
        }
    }
    return -1;
}

int TimelineEditor::markerIndexNearTimeMs(qint64 timeMs) const
{
    if (markersMs.empty()) {
        return -1;
    }
    const qint64 pxThresholdMs =
        static_cast<qint64>((kCursorMagnetPx / std::max(1.0, pixelsPerSecond())) * 1000.0);
    const qint64 thr = std::clamp<qint64>(pxThresholdMs, 10, 150);
    qint64 bestDist = thr + 1;
    int bestIdx = -1;
    for (int i = 0; i < static_cast<int>(markersMs.size()); ++i) {
        const qint64 d = std::llabs(markersMs[static_cast<size_t>(i)] - timeMs);
        if (d <= thr && d < bestDist) {
            bestDist = d;
            bestIdx = i;
        }
    }
    return bestIdx;
}

void TimelineEditor::ensureCursorVisible()
{
    const qint64 visible = contentVisibleMs();
    const qint64 margin = edgeScrollMarginMs();
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

qint64 TimelineEditor::snapTimeMsForRegion(qint64 candidateMs) const
{
    qint64 best = candidateMs;
    const qint64 pxThresholdMs =
        static_cast<qint64>((kCursorMagnetPx / std::max(1.0, pixelsPerSecond())) * 1000.0);
    const qint64 dynamicThresholdMs = std::clamp<qint64>(pxThresholdMs, 1, 8);
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
    if (dragMode != DragMode::None && dragMode != DragMode::MoveMarker) {
        const int tl = timelineLeft();
        const int tr = timelineRight();
        int xLeftEnd = tl;
        int xRightStart = tr;
        edgeScrollZoneBounds(&xLeftEnd, &xRightStart);
        const bool inLeftZone = pos.x() < tl || (pos.x() >= tl && pos.x() < xLeftEnd);
        const bool inRightZone = pos.x() > xRightStart;
        if (inLeftZone) {
            const double strength = edgeScrollStrength(pos.x(), tl, tr, xLeftEnd, xRightStart);
            const double scaled =
                (static_cast<double>(kEdgeScrollMinStepMs)
                 + static_cast<double>(kEdgeScrollMaxStepMs - kEdgeScrollMinStepMs) * strength)
                * kEdgeScrollStepScale;
            const qint64 step = std::max<qint64>(1, static_cast<qint64>(std::llround(scaled)));
            const qint64 before = timelineViewStartMs;
            setViewStartMs(timelineViewStartMs - step);
            changed = changed || (before != timelineViewStartMs);
        } else if (inRightZone) {
            const double strength = edgeScrollStrength(pos.x(), tl, tr, xLeftEnd, xRightStart);
            const double scaled =
                (static_cast<double>(kEdgeScrollMinStepMs)
                 + static_cast<double>(kEdgeScrollMaxStepMs - kEdgeScrollMinStepMs) * strength)
                * kEdgeScrollStepScale;
            const qint64 step = std::max<qint64>(1, static_cast<qint64>(std::llround(scaled)));
            const qint64 before = timelineViewStartMs;
            setViewStartMs(timelineViewStartMs + step);
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

qint64 TimelineEditor::probeSourceDurationMsUncached(const QString &path) const
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

qint64 TimelineEditor::defaultDurationForPath(const QString &path) const
{
    if (path.isEmpty()) {
        return 5000;
    }
    const auto it = sourceDurationProbeCache.constFind(path);
    if (it != sourceDurationProbeCache.constEnd()) {
        return it.value();
    }
    const qint64 ms = probeSourceDurationMsUncached(path);
    sourceDurationProbeCache.insert(path, ms);
    return ms;
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
    if (ext == "m4a" || ext == "mp3" || ext == "wav" || ext == "flac" || ext == "aac" || ext == "ogg") {
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

void TimelineEditor::notePreviewCacheHit(const QString &key)
{
    if (!previewCache.contains(key)) {
        return;
    }
    const auto it = previewCacheLruPos.find(key);
    if (it != previewCacheLruPos.end()) {
        previewCacheLru.erase(it.value());
        previewCacheLruPos.erase(it);
    }
    previewCacheLru.push_front(key);
    previewCacheLruPos.insert(key, previewCacheLru.begin());
}

void TimelineEditor::commitPreviewCacheFromWorker(const QString &key, const QImage &img)
{
    static constexpr int kMaxPreviewCacheEntries = 384;
    const auto it = previewCacheLruPos.find(key);
    if (it != previewCacheLruPos.end()) {
        previewCacheLru.erase(it.value());
        previewCacheLruPos.erase(it);
    }
    previewCache.insert(key, img);
    previewCacheLru.push_front(key);
    previewCacheLruPos.insert(key, previewCacheLru.begin());
    while (previewCacheLru.size() > kMaxPreviewCacheEntries) {
        const QString evict = previewCacheLru.back();
        previewCacheLru.pop_back();
        previewCacheLruPos.remove(evict);
        previewCache.remove(evict);
    }
}

QImage TimelineEditor::cachedVideoPreview(const QString &path, qint64 sourceInMs)
{
    const qint64 bucketMs = std::max<qint64>(0, (sourceInMs / 400) * 400);
    const QString cacheKey = QString("%1::%2").arg(path, QString::number(bucketMs));
    const auto it = previewCache.constFind(cacheKey);
    if (it != previewCache.constEnd()) {
        notePreviewCacheHit(cacheKey);
        return it.value();
    }

    if (pendingPreviewKeys.contains(cacheKey)) {
        return QImage();
    }
    // Prevent preview decoder flood while timeline is repainting rapidly.
    if (pendingPreviewKeys.size() >= 48) {
        static std::atomic<qint64> sLastStripQueueDiagMs{0};
        const qint64 now = QDateTime::currentMSecsSinceEpoch();
        qint64 prev = sLastStripQueueDiagMs.load();
        if (now - prev > 4000 && sLastStripQueueDiagMs.compare_exchange_strong(prev, now)) {
            burstcon::reportBurstconDiagnostic(
                QStringLiteral("strip thumbnails: pending preview keys >= 48 (queue saturated); "
                               "increase interval or reduce repaint rate."));
        }
        return QImage();
    }
    if (gStripPreviewDecodeJobs.load() >= 16) {
        static std::atomic<qint64> sLastStripJobsDiagMs{0};
        const qint64 now = QDateTime::currentMSecsSinceEpoch();
        qint64 prev = sLastStripJobsDiagMs.load();
        if (now - prev > 4000 && sLastStripJobsDiagMs.compare_exchange_strong(prev, now)) {
            burstcon::reportBurstconDiagnostic(
                QStringLiteral("strip thumbnails: concurrent decode jobs >= 16 (cap hit)."));
        }
        return QImage();
    }
    pendingPreviewKeys.insert(cacheKey);
    gStripPreviewDecodeJobs.fetch_add(1);

    QPointer<TimelineEditor> guard(this);
    QtConcurrent::run([guard, path, bucketMs, cacheKey]() {
        if (!guard) {
            gStripPreviewDecodeJobs.fetch_sub(1);
            return;
        }
        QImage img = guard->decodeVideoPreviewSync(path, bucketMs);
        const QSize target(320, 180);
        const QImage scaled = (img.size() == target)
            ? img
            : img.scaled(target, Qt::KeepAspectRatioByExpanding, Qt::FastTransformation);
        if (!guard) {
            gStripPreviewDecodeJobs.fetch_sub(1);
            return;
        }
        QMetaObject::invokeMethod(
            guard.data(),
            [guard, cacheKey, scaled]() {
                if (!guard) {
                    gStripPreviewDecodeJobs.fetch_sub(1);
                    return;
                }
                guard->commitPreviewCacheFromWorker(cacheKey, scaled);
                guard->pendingPreviewKeys.remove(cacheKey);
                gStripPreviewDecodeJobs.fetch_sub(1);
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
        const auto pool = stripMediaPool ? stripMediaPool : playbackMediaPool;
        if (pool) {
            // `forPreview: true` uses a separate ConMedia pool key (`p:`) so timeline thumbnails do not
            // time-share the playback decoder (`m:`) on the same path.
            const burstcon::BurstIndex originIdx = burstcon::msToBurstIndex(bucketMs);
            size_t poolShard = 0;
            if (stripMediaPool) {
                const size_t n = stripMediaPool->previewParallelShardCount();
                if (n > 1) {
                    const quint64 h = static_cast<quint64>(qHash(path)) ^
                        (static_cast<quint64>(static_cast<qint64>(bucketMs)) * 0x9e3779b97f4a7c15ULL);
                    poolShard = 1 + static_cast<size_t>(h % n);
                }
            }
            auto media = pool->acquire(path, true, false, originIdx, true, false, poolShard);
            burstcon::BurstRequest req;
            req.index = burstcon::msToBurstIndex(bucketMs);
            req.quality = burstcon::BurstQuality{320, 180};
            req.decodeOrigin = originIdx;
            burstcon::VideoBurst frame;
            if (media && media->videoBurst(req, frame) && !frame.frame.isNull()) {
                img = frame.frame;
            } else if (media) {
                static std::atomic<qint64> sLastPreviewFailMs{0};
                const qint64 now = QDateTime::currentMSecsSinceEpoch();
                qint64 prev = sLastPreviewFailMs.load();
                if (now - prev > 2000 && sLastPreviewFailMs.compare_exchange_strong(prev, now)) {
                    burstcon::reportBurstconDiagnostic(
                        QStringLiteral("strip preview decode miss path=%1 bucketMs=%2 idx=%3")
                            .arg(path)
                            .arg(bucketMs)
                            .arg(req.index));
                }
            } else {
                static std::atomic<qint64> sLastAcquireFailMs{0};
                const qint64 now = QDateTime::currentMSecsSinceEpoch();
                qint64 prev = sLastAcquireFailMs.load();
                if (now - prev > 5000 && sLastAcquireFailMs.compare_exchange_strong(prev, now)) {
                    burstcon::reportBurstconDiagnostic(
                        QStringLiteral("strip preview acquire returned null path=%1").arg(path));
                }
            }
        }
        if (img.isNull() && isImagePath(path)) {
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

QString TimelineEditor::peaksCacheKey(const QString &path,
                                       qint64 sourceInMs,
                                       qint64 durationMs,
                                       qint64 sourceDurationMs,
                                       int sampleCount)
{
    return QStringLiteral("%1|%2|%3|%4|%5")
        .arg(path)
        .arg(sourceInMs)
        .arg(durationMs)
        .arg(sourceDurationMs)
        .arg(sampleCount);
}

void TimelineEditor::commitPeaksCacheFromWorker(const QString &key, const QVector<QPair<int, int>> &peaks)
{
    if (!key.isEmpty()) {
        peaksStereoCache.insert(key, peaks);
    }
}

QVector<QPair<int, int>> TimelineEditor::decodeAudioPeaksStereoSync(const QString &path,
                                                                    qint64 sourceInMs,
                                                                    qint64 durationMs,
                                                                    int peakColumns) const
{
    QVector<QPair<int, int>> out;
    if (path.isEmpty() || peakColumns <= 0 || durationMs <= 0) {
        return out;
    }
    const auto pool = stripMediaPool ? stripMediaPool : playbackMediaPool;
    if (!pool) {
        return out;
    }
    auto media = pool->acquire(path, true, false, burstcon::msToBurstIndex(sourceInMs), false, true);
    if (!media) {
        return out;
    }
    constexpr int kRate = 48000;
    constexpr int kSpb = 1024;
    const burstcon::BurstIndex audioStep =
        std::max<burstcon::BurstIndex>(1, (burstcon::kBurstTimebaseHz * static_cast<burstcon::BurstIndex>(kSpb)) / kRate);
    const burstcon::BurstIndex originIdx = burstcon::msToBurstIndex(sourceInMs);
    const int cols = std::clamp(peakColumns, 8, 256);
    out.resize(cols);
    const qint64 dur = std::max<qint64>(1, durationMs);
    for (int i = 0; i < cols; ++i) {
        const qint64 offset = (cols <= 1) ? 0 : (dur * static_cast<qint64>(i)) / static_cast<qint64>(cols - 1);
        const qint64 srcMs = sourceInMs + std::min(offset, dur - 1);
        const burstcon::BurstIndex rawIdx = burstcon::msToBurstIndex(srcMs);
        const burstcon::BurstIndex idx = (rawIdx / audioStep) * audioStep;
        burstcon::BurstRequest req;
        req.index = idx;
        req.decodeOrigin = originIdx;
        burstcon::AudioBurst ab;
        if (!media->audioBurst(req, ab) || ab.samples.empty()) {
            out[i] = {1, 1};
            continue;
        }
        const int ch = std::max(1, ab.channels);
        int peakL = 0;
        int peakR = 0;
        for (size_t s = 0; s + static_cast<size_t>(ch) <= ab.samples.size(); s += static_cast<size_t>(ch)) {
            const int v0 = static_cast<int>(std::clamp(ab.samples[s],
                                                       static_cast<int32_t>(std::numeric_limits<int16_t>::min()),
                                                       static_cast<int32_t>(std::numeric_limits<int16_t>::max())));
            peakL = std::max(peakL, std::abs(v0));
            if (ch >= 2) {
                const int v1 = static_cast<int>(std::clamp(ab.samples[s + 1],
                                                           static_cast<int32_t>(std::numeric_limits<int16_t>::min()),
                                                           static_cast<int32_t>(std::numeric_limits<int16_t>::max())));
                peakR = std::max(peakR, std::abs(v1));
            } else {
                peakR = peakL;
            }
        }
        const int nL = std::clamp(1 + (peakL * 799) / 32768, 1, 800);
        const int nR = std::clamp(1 + (peakR * 799) / 32768, 1, 800);
        out[i] = {nL, nR};
    }
    int m = 1;
    for (const auto &pr : out) {
        m = std::max(m, std::max(pr.first, pr.second));
    }
    if (m > 1) {
        for (auto &pr : out) {
            pr.first = std::max(1, (pr.first * 500) / m);
            pr.second = std::max(1, (pr.second * 500) / m);
        }
    }
    return out;
}

QVector<QPair<int, int>> TimelineEditor::cachedAudioPeaksStereo(const QString &path,
                                                                qint64 sourceInMs,
                                                                qint64 durationMs,
                                                                qint64 sourceDurationMs,
                                                                int sampleCount)
{
    if (sampleCount <= 0 || path.isEmpty() || !QFileInfo::exists(path)) {
        return {};
    }
    // Quantize column count so small timeline scrolls / width tweaks do not invalidate the cache.
    const int peakCols = std::clamp(((std::max(1, sampleCount) + 31) / 32) * 32, 32, 256);
    const QString key = peaksCacheKey(path, sourceInMs, durationMs, sourceDurationMs, peakCols);
    const auto it = peaksStereoCache.constFind(key);
    if (it != peaksStereoCache.constEnd()) {
        return it.value();
    }
    if (pendingPeaksKeys.contains(key)) {
        return {};
    }
    if (pendingPeaksKeys.size() >= 8) {
        return {};
    }
    pendingPeaksKeys.insert(key);
    QPointer<TimelineEditor> guard(this);
    QtConcurrent::run([guard, path, sourceInMs, durationMs, peakCols, key]() {
        QVector<QPair<int, int>> built;
        if (guard) {
            built = guard->decodeAudioPeaksStereoSync(path, sourceInMs, durationMs, peakCols);
        }
        if (!guard) {
            return;
        }
        const QVector<QPair<int, int>> copy = built;
        QTimer::singleShot(0, guard.data(), [guard, key, copy]() {
            if (!guard) {
                return;
            }
            guard->commitPeaksCacheFromWorker(key, copy);
            guard->pendingPeaksKeys.remove(key);
            guard->update();
        });
    });
    return {};
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
    QString pendingText;
    if (pendingMorpherPlacement.active) {
        pendingText = tr(" | Morpher: click purple Morph track to place (Esc = cancel)");
    }
    const QString text = QString("Time: %1 | Zoom: %2x | Video layers: %3 | Audio layers: %4 | %5")
                             .arg(formatTimelineTime(cursorPositionMs))
                             .arg(zoomLevel, 0, 'f', 2)
                             .arg(activeVideoCount)
                             .arg(activeAudioCount)
                             .arg(regionText + (loopRegionEnabled ? " (loop)" : "") + pendingText);
    if (text != lastOverlayInfo) {
        lastOverlayInfo = text;
        emit overlayInfoChanged(text);
    }
}
