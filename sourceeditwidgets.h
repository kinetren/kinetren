#ifndef SOURCEEDITWIDGETS_H
#define SOURCEEDITWIDGETS_H

#include <QWidget>
#include <QString>

class QLineEdit;
class QDoubleSpinBox;
class QLabel;
class QPushButton;
class QDragEnterEvent;
class QDropEvent;
class TimelineEditor;

class SourceReplacePanel : public QWidget
{
    Q_OBJECT
public:
    explicit SourceReplacePanel(TimelineEditor *timeline, QWidget *parent = nullptr);

    void setClipTarget(int clipId);
    void setRegionTarget(bool allTracks, int trackIndex);
    void setMorpherTarget(int morpherEventId);
    void clearTarget();

protected:
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dropEvent(QDropEvent *event) override;

private slots:
    void onBrowse();
    void onApply();

private:
    void refreshUi();
    void applyPath(const QString &path);

    TimelineEditor *timeline = nullptr;
    enum class TargetKind { None, Clip, Region, Morpher };
    TargetKind kind = TargetKind::None;
    int targetClipId = -1;
    bool regionAllTracks = false;
    int regionTrackIndex = -1;
    int targetMorpherId = -1;

    QLabel *titleLabel = nullptr;
    QLineEdit *pathEdit = nullptr;
    QPushButton *browseButton = nullptr;
    QPushButton *applyButton = nullptr;
};

class MorpherEditWindow : public QWidget
{
    Q_OBJECT
public:
    explicit MorpherEditWindow(TimelineEditor *timeline, int morpherEventId, QWidget *parent = nullptr);
    int morpherEventId() const { return morpherId; }

public slots:
    void reloadFromTimeline();

private slots:
    void onBrowse();
    void onApplyReplace();
    void onApplyOffset();

protected:
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dropEvent(QDropEvent *event) override;

private:
    void applyPath(const QString &path);

    TimelineEditor *timeline = nullptr;
    int morpherId = -1;
    QLabel *targetsLabel = nullptr;
    QLineEdit *pathEdit = nullptr;
    QPushButton *browseButton = nullptr;
    QPushButton *applyReplaceButton = nullptr;
    QDoubleSpinBox *deltaSpin = nullptr;
    QPushButton *applyOffsetButton = nullptr;
};

class SourceOffsetPanel : public QWidget
{
    Q_OBJECT
public:
    explicit SourceOffsetPanel(TimelineEditor *timeline, QWidget *parent = nullptr);

    void setClipTarget(int clipId);
    void setRegionShiftTarget(bool allTracks, int trackIndex);
    void setMorpherTarget(int morpherEventId);
    void clearTarget();

private slots:
    void onApply();

private:
    void refreshUi();

    TimelineEditor *timeline = nullptr;
    enum class TargetKind { None, Clip, RegionShift, Morpher };
    TargetKind kind = TargetKind::None;
    int targetClipId = -1;
    bool regionAllTracks = false;
    int regionTrackIndex = -1;
    int targetMorpherId = -1;

    QLabel *titleLabel = nullptr;
    QDoubleSpinBox *valueSpin = nullptr;
    QPushButton *applyButton = nullptr;
};

#endif
