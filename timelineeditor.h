#ifndef TIMELINEEDITOR_H
#define TIMELINEEDITOR_H

#include <QWidget>
#include <QRect>
#include <QColor>
#include <QPoint>
#include <QHash>
#include <QImage>
#include <QQueue>
#include <QSet>
#include <QWheelEvent>
#include <QEvent>

#include <optional>
#include <vector>

class QJsonObject;

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
    };

    explicit TimelineEditor(QWidget *parent = nullptr);

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
    void setCacheCoverage(const QHash<QString, QVector<QPair<double, double>>> &clipSegments,
                          const QVector<QPair<double, double>> &timelineSegments);
    QJsonObject toJson() const;
    bool fromJson(const QJsonObject &root);

signals:
    void cursorChanged(qint64 cursorMs);
    void overlayInfoChanged(const QString &text);
    void timelineRangeChanged(qint64 maxTimelineMs, qint64 visibleDurationMs);
    void viewStartChanged(qint64 viewStartMs, qint64 maxViewStartMs);
    void loopEnabledChanged(bool enabled);
    void selectionRegionChanged(bool hasRegion, qint64 startMs, qint64 endMs);
    void timelineContentChanged();

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

private:
    enum class TrackType {
        Video,
        Audio
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
    };

    struct TrackLane {
        TrackType type = TrackType::Video;
        QString name;
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
        MoveMarker
    };

    std::vector<TrackLane> tracks;
    std::vector<ClipEvent> clips;
    std::vector<ClipEvent> clipboardClips;
    std::vector<qint64> markersMs;
    int nextClipId = 1;
    int nextLinkedGroupId = 1;
    qint64 cursorPositionMs = 0;
    int selectedClipId = -1;
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
    bool isImagePath(const QString &path) const;
    bool hasVideoForPath(const QString &path) const;
    bool hasAudioForPath(const QString &path) const;
    QColor colorFromPath(const QString &path) const;
    QImage cachedVideoPreview(const QString &path, qint64 sourceInMs);
    QImage decodeVideoPreviewSync(const QString &path, qint64 bucketMs) const;
    std::vector<int> cachedAudioPeaks(const QString &path,
                                      qint64 sourceInMs,
                                      qint64 durationMs,
                                      qint64 sourceDurationMs,
                                      int sampleCount);
    void unlinkClipGroup(int clipId);
    std::vector<int> linkedClipIds(int clipId) const;

    void addClipFromDrop(const QString &path, qint64 startMs, int dropY);
    void emitOverlayInfo();
    QHash<QString, QImage> previewCache;
    QQueue<QString> previewCacheOrder;
    QSet<QString> pendingPreviewKeys;
    QHash<QString, std::vector<int>> peaksCache;
    QHash<QString, QVector<QPair<double, double>>> cacheSegmentsByClip;
    QVector<QPair<double, double>> timelineCacheSegments;
};

#endif // TIMELINEEDITOR_H
