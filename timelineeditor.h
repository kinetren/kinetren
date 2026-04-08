#ifndef TIMELINEEDITOR_H
#define TIMELINEEDITOR_H

#include <QWidget>

#include <optional>
#include <vector>

class TimelineEditor : public QWidget
{
    Q_OBJECT

public:
    struct ActiveClip {
        QString sourcePath;
        qint64 startMs = 0;
        qint64 durationMs = 0;
    };

    explicit TimelineEditor(QWidget *parent = nullptr);

    qint64 cursorMs() const;
    void setCursorMs(qint64 valueMs);
    void addMarkerAtCursor();
    std::optional<ActiveClip> activeVideoAt(qint64 timeMs) const;
    std::vector<ActiveClip> activeAudioAt(qint64 timeMs) const;

signals:
    void cursorChanged(qint64 cursorMs);
    void overlayInfoChanged(const QString &text);

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dragMoveEvent(QDragMoveEvent *event) override;
    void dropEvent(QDropEvent *event) override;

private:
    enum class TrackType {
        Video,
        Audio
    };

    struct ClipEvent {
        int id = 0;
        int linkedGroupId = -1;
        TrackType track = TrackType::Video;
        qint64 startMs = 0;
        qint64 durationMs = 0;
        QString sourcePath;
        QColor color;
    };

    std::vector<ClipEvent> clips;
    std::vector<qint64> markersMs;
    int nextClipId = 1;
    int nextLinkedGroupId = 1;
    qint64 cursorPositionMs = 0;
    int selectedClipId = -1;
    QString lastOverlayInfo;

    int timelineLeft() const;
    int timelineRight() const;
    int rulerTop() const;
    int rulerHeight() const;
    int videoTrackTop() const;
    int audioTrackTop() const;
    int trackHeight() const;
    int bottomPadding() const;
    double pixelsPerSecond() const;

    qint64 xToTimeMs(int x) const;
    int timeToX(qint64 timeMs) const;
    int trackAtY(int y) const;
    qint64 snapTimeMs(qint64 candidateMs) const;
    qint64 defaultDurationForPath(const QString &path) const;
    bool isImagePath(const QString &path) const;
    bool hasVideoForPath(const QString &path) const;
    bool hasAudioForPath(const QString &path) const;
    QColor colorFromPath(const QString &path) const;

    void addClipFromDrop(const QString &path, qint64 startMs);
    void emitOverlayInfo();
};

#endif // TIMELINEEDITOR_H
