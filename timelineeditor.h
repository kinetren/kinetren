#ifndef TIMELINEEDITOR_H
#define TIMELINEEDITOR_H

#include <QWidget>
#include <QRect>
#include <QColor>
#include <QPoint>
#include <QHash>
#include <QImage>
#include <QSet>
#include <QKeyEvent>
#include <QWheelEvent>
#include <QResizeEvent>
#include <QEvent>

#include <list>
#include <memory>
#include <optional>
#include <unordered_set>
#include <vector>

#include <QVector>
#include <QJsonArray>

namespace burstcon {
class ConMediaPool;
}

class QJsonObject;
class QMimeData;

class TimelineEditor : public QWidget
{
    Q_OBJECT

public:
    struct ActiveClip {
        QString sourcePath;
        qint64 startMs = 0;
        qint64 durationMs = 0;
        qint64 sourceInMs = 0;
    };
    struct RenderClip {
        QString sourcePath;
        qint64 startMs = 0;
        qint64 durationMs = 0;
        qint64 sourceInMs = 0;
        int trackIndex = 0;
        bool isVideo = false;
        bool isAudio = false;
        QJsonArray videoFxChain;
        QJsonArray audioFxChain;
        float audioLinearGain = 1.f;
    };

    explicit TimelineEditor(QWidget *parent = nullptr);
    ~TimelineEditor() override;

    /// Strip thumbnails decode through this pool (same as PlaybackPipeline) so each file is indexed once.
    void setPlaybackMediaPool(std::shared_ptr<burstcon::ConMediaPool> pool);
    /// Dedicated pool for strip thumbnails / waveform probes so they do not share decoder instances with playback.
    void setStripMediaPool(std::shared_ptr<burstcon::ConMediaPool> pool);
    /// Drop strip thumbnail / waveform caches for one path (e.g. media restored after offline).
    void invalidateSourcePath(const QString &path);
    /// Drop strip thumbnails, peaks, and coverage segment caches (e.g. after preview scale mode changes).
    void purgePresentationCaches();

    qint64 cursorMs() const;
    void setCursorMs(qint64 valueMs, bool ensureVisible = true);
    void addMarkerAtCursor();
    void addVideoTrack();
    void addAudioTrack();
    bool splitSelectedClipAtCursor();
    bool trimSelectedClipStartToCursor();
    bool trimSelectedClipEndToCursor();
    bool moveSelectedClipToAdjacentTrack(int direction);
    bool copySelectedToClipboard();
    bool pasteClipboardAtCursor();
    void setLoopEnabled(bool enabled);
    bool loopEnabled() const;
    bool hasSelectionRegion() const;
    qint64 selectionRegionStartMs() const;
    qint64 selectionRegionEndMs() const;
    void setSelectionRegionFromCursor(bool setStart);
    void clearSelectionRegion();
    bool deleteSelectedClip();
    bool deleteClipsIntersectingRegion();
    bool splitDeleteRegionInterior();
    bool magnetizeSelectedClip();
    qint64 maxTimelineMs() const;
    qint64 visibleDurationMs() const;
    qint64 viewStartMs() const;
    qint64 maxViewStartMs() const;
    void setViewStartMs(qint64 valueMs);
    void setZoomFactor(double value);
    void setZoomFactor(double value, qint64 anchorTimeMs);
    double zoomFactor() const;
    std::optional<ActiveClip> activeVideoAt(qint64 timeMs) const;
    std::vector<ActiveClip> activeVideoLayersAt(qint64 timeMs) const;
    std::vector<ActiveClip> activeAudioAt(qint64 timeMs) const;
    std::vector<RenderClip> renderClips() const;
    QString formatTimelineTime(qint64 timeMs) const;
    void setTimelineFps(int fps);
    bool isEditingTimelineContent() const;
    void setCacheCoverage(const QHash<QString, QVector<QPair<double, double>>> &clipVideoSliding,
                          const QHash<QString, QVector<QPair<double, double>>> &clipVideoHead,
                          const QVector<QPair<double, double>> &timelineVideoSliding,
                          const QVector<QPair<double, double>> &timelineVideoHead,
                          const QHash<QString, QVector<QPair<double, double>>> &clipAudioSegments);
    QJsonObject toJson() const;
    bool fromJson(const QJsonObject &root);

    void applyReplaceSourceToClip(int clipId, const QString &path);
    void applyReplaceSourceToRegion(const QString &path, bool allTracks, int trackIndex);
    void applyAbsoluteSourceStartToClip(int clipId, qint64 sourceInMs);
    void applyRegionSourceShift(qint64 deltaMs, bool allTracks, int trackIndex);

    QString clipSourcePath(int clipId) const;
    qint64 clipSourceInMs(int clipId) const;
    QJsonArray clipVideoFxChain(int clipId) const;
    void setClipVideoFxChain(int clipId, const QJsonArray &chain);
    QJsonArray clipAudioFxChain(int clipId) const;
    void setClipAudioFxChain(int clipId, const QJsonArray &chain);
    float clipAudioVolumeUiT(int clipId) const;
    void setClipAudioVolumeUiT(int clipId, float t01);
    QImage previewStillForClip(int clipId);

    static QString firstLocalPathFromMime(const QMimeData *mime);

    void addMorpherTrack();
    void beginMorpherPlacementFromSelection();
    void bakeMorpherEvent(int morpherEventId);
    void deleteMorpherEvent(int morpherEventId);
    int selectedMorpherEventId() const;
    QString morpherReplacePath(int morpherEventId) const;
    void setMorpherReplacePath(int morpherEventId, const QString &path);
    qint64 morpherSourceInDeltaMs(int morpherEventId) const;
    void setMorpherSourceInDeltaMs(int morpherEventId, qint64 deltaMs);
    qint64 morpherAbsoluteSourceInMs(int morpherEventId) const;
    void setMorpherAbsoluteSourceInMs(int morpherEventId, qint64 ms); // pass -1 to clear
    void deleteCurrentSelection();
    bool hasMorpherEvent(int morpherEventId) const;
    QString morpherTargetsDescription(int morpherEventId) const;

    enum class MorpherKind { ReplaceSource, EffectBatch };
    void beginMorpherEffectBatchPlacementFromSelection();
    MorpherKind morpherEventKind(int morpherEventId) const;
    QJsonArray morpherBulkVideoFxAppend(int morpherEventId) const;
    QJsonArray morpherBulkAudioFxAppend(int morpherEventId) const;
    void setMorpherBulkVideoFxAppend(int morpherEventId, const QJsonArray &chain);
    void setMorpherBulkAudioFxAppend(int morpherEventId, const QJsonArray &chain);

    /// True if clip exists and matches the FX editor kind (video vs audio).
    bool clipMatchesFxTarget(int clipId, bool wantVideoFx) const;

    int timelineContentHeightPx() const;
    int timelineVerticalScroll() const;
    void setTimelineVerticalScroll(int px);
    /// Width of the left track-name column (align external chrome with the timeline).
    int timelineTrackHeaderWidth() const { return headerWidth(); }

    /// Path classification (used by MainWindow / pool probing; same rules as drop-to-timeline).
    bool isImagePath(const QString &path) const;
    bool hasVideoForPath(const QString &path) const;
    bool hasAudioForPath(const QString &path) const;

signals:
    void cursorChanged(qint64 cursorMs);
    void overlayInfoChanged(const QString &text);
    void timelineRangeChanged(qint64 maxTimelineMs, qint64 visibleDurationMs);
    void viewStartChanged(qint64 viewStartMs, qint64 maxViewStartMs);
    void loopEnabledChanged(bool enabled);
    void selectionRegionChanged(bool hasRegion, qint64 startMs, qint64 endMs);
    void timelineContentChanged();
    void replaceSourcePanelRequestedForClip(int clipId);
    void replaceSourcePanelRequestedForRegion(bool allTracks, int trackIndex);
    void sourceOffsetPanelRequestedForClip(int clipId);
    void sourceOffsetPanelRequestedForRegion(bool allTracks, int trackIndex);
    void replaceSourcePanelRequestedForMorpher(int morpherEventId);
    void sourceOffsetPanelRequestedForMorpher(int morpherEventId);
    void morpherEditWindowRequested(int morpherEventId);
    void clipVideoFxEditorRequested(int clipId, const QString &focusHint);
    void clipAudioFxEditorRequested(int clipId);
    void timelineViewLayoutChanged();
    void timelineVerticalScrollChanged(int px);

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseDoubleClickEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void contextMenuEvent(QContextMenuEvent *event) override;
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dragMoveEvent(QDragMoveEvent *event) override;
    void dropEvent(QDropEvent *event) override;
    void wheelEvent(QWheelEvent *event) override;
    void leaveEvent(QEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;

private:
    enum class TrackType {
        Video,
        Audio,
        Morpher
    };

    struct ClipEvent {
        int id = 0;
        int linkedGroupId = -1;
        TrackType mediaType = TrackType::Video;
        int trackIndex = 0;
        qint64 startMs = 0;
        qint64 durationMs = 0;
        qint64 sourceInMs = 0;
        qint64 sourceDurationMs = 0;
        QString sourcePath;
        QColor color;
        QJsonArray videoFxChain;
        QJsonArray audioFxChain;
        /// 0 = top of clip (boost), 1 = bottom (cut); 0.5 = unity gain.
        float audioVolumeUiT = 0.5f;
    };

    struct TrackLane {
        TrackType type = TrackType::Video;
        QString name;
    };

    struct MorpherEvent {
        int id = 0;
        int trackIndex = 0;
        qint64 startMs = 0;
        qint64 durationMs = 1500;
        QVector<int> targetClipIds;
        MorpherKind kind = MorpherKind::ReplaceSource;
        QString newSourcePath;
        qint64 sourceInDeltaMs = 0;
        qint64 absoluteSourceInMs = -1;
        QJsonArray bulkVideoFxAppend;
        QJsonArray bulkAudioFxAppend;
    };

    enum class DragMode {
        None,
        ScrubCursor,
        MoveClip,
        TrimStart,
        TrimEnd,
        RegionSelect,
        MoveRegionStart,
        MoveRegionEnd,
        MoveMarker,
        AdjustAudioVolume
    };

    std::vector<TrackLane> tracks;
    std::vector<ClipEvent> clips;
    std::vector<MorpherEvent> morpherEvents;
    std::vector<ClipEvent> clipboardClips;
    std::vector<qint64> markersMs;
    int nextClipId = 1;
    int nextLinkedGroupId = 1;
    int nextMorpherEventId = 1;
    qint64 cursorPositionMs = 0;
    int selectedClipId = -1;
    int selectedMorpherId = -1;
    qint64 timelineViewStartMs = 0;
    bool loopRegionEnabled = false;
    bool hasRegionSelection = false;
    qint64 regionStartMs = 0;
    qint64 regionEndMs = 0;
    DragMode dragMode = DragMode::None;
    int dragClipId = -1;
    qint64 dragAnchorMs = 0;
    qint64 dragOriginalStartMs = 0;
    int dragOriginalTrackIndex = -1;
    qint64 trimOriginalDurationMs = 0;
    qint64 trimOriginalSourceInMs = 0;
    qint64 dragLastPointerMs = 0;
    qint64 dragStartPointerMs = 0;
    bool dragChangedTimelineContent = false;
    QHash<int, qint64> dragInitialStarts;
    QHash<int, qint64> dragInitialDurations;
    QHash<int, qint64> dragInitialSourceIns;
    qint64 regionAnchorMs = 0;
    int dragMarkerIndex = -1;
    QPoint lastHoverPos{-1, -1};
    QString lastOverlayInfo;
    double zoomLevel = 1.0;
    int timelineFps = 30;
    int verticalScrollPx = 0;

    struct PendingMorpherPlacement {
        bool active = false;
        QVector<int> targetClipIds;
        MorpherKind pendingKind = MorpherKind::ReplaceSource;
    };
    PendingMorpherPlacement pendingMorpherPlacement;

    void clearPendingMorpherPlacement();
    void beginMorpherPlacementCommon(MorpherKind kind);
    QVector<int> collectMorpherTargetClipIds();
    void commitPendingMorpherPlacement(int trackIndex, qint64 startMs);

    ClipEvent effectiveClipForRender(const ClipEvent &base) const;
    void applyMorpherPatchToClipCopy(ClipEvent &clip, const MorpherEvent &m) const;
    MorpherEvent *findMorpherById(int id);
    const MorpherEvent *findMorpherById(int id) const;
    QRect morpherEventRect(const MorpherEvent &m) const;
    int morpherEventIdAt(const QPoint &pos) const;

    int timelineLeft() const;
    int timelineRight() const;
    int rulerTop() const;
    int rulerHeight() const;
    int trackHeight() const;
    int trackGap() const;
    int headerWidth() const;
    int trackTop(int trackIndex) const;
    int contentBottom() const;
    int bottomPadding() const;
    QPoint mapToTimelineContent(const QPoint &widgetPos) const;
    void clampVerticalScroll();
    void contentGeometryUpdated();
    double pixelsPerSecond() const;

    qint64 xToTimeMs(int x) const;
    int timeToX(qint64 timeMs) const;
    int trackIndexAtY(int y) const;
    int insertTrackAt(int index, TrackType type);
    int ensureDropTrackForType(TrackType type, int dropY);
    int maxTrackOfType(TrackType type) const;
    bool isTrackIndexValid(int trackIndex) const;
    QRect clipRect(const ClipEvent &clip) const;
    ClipEvent *findClipById(int id);
    const ClipEvent *findClipById(int id) const;
    int clipIdAt(const QPoint &pos) const;
    bool isNearClipStartEdge(const ClipEvent &clip, int x) const;
    bool isNearClipEndEdge(const ClipEvent &clip, int x) const;
    bool hasMatchingLinkedVideo(const ClipEvent &clip) const;
    bool shouldDrawClipLabel(const ClipEvent &clip) const;
    QString clipCoverageKey(const ClipEvent &clip) const;
    bool isNearRegionStartBoundary(int x) const;
    bool isNearRegionEndBoundary(int x) const;
    qint64 contentVisibleMs() const;
    qint64 recomputeMaxTimelineMs() const;
    void setRegionBounds(qint64 startMs, qint64 endMs);
    void emitRegionChanged();
    void ensureCursorVisible();
    qint64 edgeScrollMarginMs() const;
    void edgeScrollZoneBounds(int *xLeftEnd, int *xRightStart) const;
    int markerIndexAt(const QPoint &pos) const;
    int markerIndexNearTimeMs(qint64 timeMs) const;
    double edgeScrollStrength(int mouseX, int tl, int tr, int xLeftEnd, int xRightStart) const;
    void refreshTimelineRangeSignals();
    qint64 snapTimeMs(qint64 candidateMs) const;
    qint64 snapTimeMsForRegion(qint64 candidateMs) const;
    qint64 adjustedDragPointerMs(const QPoint &pos, bool *scrolled = nullptr);
    qint64 timelineTickStepMs() const;
    qint64 defaultDurationForPath(const QString &path) const;
    qint64 probeSourceDurationMsUncached(const QString &path) const;
    QColor colorFromPath(const QString &path) const;
    QImage cachedVideoPreview(const QString &path, qint64 sourceInMs);
    QImage decodeVideoPreviewSync(const QString &path, qint64 bucketMs) const;
    void notePreviewCacheHit(const QString &key);
    void commitPreviewCacheFromWorker(const QString &key, const QImage &img);
    QVector<QPair<int, int>> cachedAudioPeaksStereo(const QString &path,
                                                    qint64 sourceInMs,
                                                    qint64 durationMs,
                                                    qint64 sourceDurationMs,
                                                    int sampleCount);
    QVector<QPair<int, int>> decodeAudioPeaksStereoSync(const QString &path,
                                                        qint64 sourceInMs,
                                                        qint64 durationMs,
                                                        int peakColumns) const;
    void commitPeaksCacheFromWorker(const QString &key, const QVector<QPair<int, int>> &peaks);
    static QString peaksCacheKey(const QString &path,
                                 qint64 sourceInMs,
                                 qint64 durationMs,
                                 qint64 sourceDurationMs,
                                 int sampleCount);
    void unlinkClipGroup(int clipId);
    std::vector<int> linkedClipIds(int clipId) const;
    std::unordered_set<int> clipIdsIntersectingRegion(bool allTracks, int trackIndex) const;
    bool applyNewSourcePathToClip(ClipEvent &clip, const QString &path) const;
    void replaceSourceForLinkedClipGroup(int clipId, const QString &path);
    void setSourceStartMsForLinkedClipGroup(int clipId, qint64 sourceInMs);
    void replaceSourceForRegionClips(const QString &path, bool allTracks, int trackIndex);
    void shiftSourceStartForRegionClips(qint64 deltaMs, bool allTracks, int trackIndex);

    void addClipFromDrop(const QString &path, qint64 startMs, int dropY);
    enum class ClipChromeHit { None, VideoFx, VideoPan, AudioFx, AudioVolumeRail };
    ClipChromeHit clipChromeHit(const QPoint &timelinePos, int *outClipId) const;
    static QRect videoFxButtonRect(const QRect &clipR);
    static QRect videoPanButtonRect(const QRect &clipR);
    static QRect audioFxButtonRect(const QRect &clipR);
    static QRect audioVolumeRailRect(const QRect &clipR);
    void ensureClipFxDefaults(ClipEvent &clip);
    void emitOverlayInfo();
    QHash<QString, QImage> previewCache;
    std::list<QString> previewCacheLru;
    QHash<QString, std::list<QString>::iterator> previewCacheLruPos;
    QSet<QString> pendingPreviewKeys;
    QSet<QString> pendingPeaksKeys;
    QHash<QString, QVector<QPair<int, int>>> peaksStereoCache;
    QHash<QString, QVector<QPair<double, double>>> cacheVideoSlidingByClip;
    QHash<QString, QVector<QPair<double, double>>> cacheVideoHeadByClip;
    QVector<QPair<double, double>> timelineVideoSlidingSegments;
    QVector<QPair<double, double>> timelineVideoHeadSegments;
    QHash<QString, QVector<QPair<double, double>>> cacheAudioByClip;
    /// Avoid probing the same path repeatedly (bulk replace, morpher preview paint).
    mutable QHash<QString, qint64> sourceDurationProbeCache;
    std::shared_ptr<burstcon::ConMediaPool> playbackMediaPool;
    std::shared_ptr<burstcon::ConMediaPool> stripMediaPool;
};

#endif // TIMELINEEDITOR_H
