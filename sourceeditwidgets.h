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
class FxChainEditorWidget;

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
    bool eventFilter(QObject *watched, QEvent *event) override;

private slots:
    void onBrowse();
    void onPathEditChanged(const QString &text);

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
};

class MorpherEditWindow : public QWidget
{
    Q_OBJECT
public:
    explicit MorpherEditWindow(TimelineEditor *timeline, int morpherEventId, QWidget *parent = nullptr);
    int morpherEventId() const { return morpherId; }

public slots:
    void reloadFromTimeline();

protected:
    void changeEvent(QEvent *event) override;
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dropEvent(QDropEvent *event) override;
    bool eventFilter(QObject *watched, QEvent *event) override;

private slots:
    void onBrowse();
    void onPathEditChanged(const QString &text);
    void onDeltaSpinChanged(double value);

private:
    void applyPath(const QString &path);

    TimelineEditor *timeline = nullptr;
    int morpherId = -1;
    QLabel *targetsLabel = nullptr;
    QWidget *replaceModeWidget = nullptr;
    QWidget *fxBatchWidget = nullptr;
    FxChainEditorWidget *vfxEditor = nullptr;
    FxChainEditorWidget *afxEditor = nullptr;
    QLineEdit *pathEdit = nullptr;
    QPushButton *browseButton = nullptr;
    QDoubleSpinBox *deltaSpin = nullptr;
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
    void onOffsetSpinChanged(double value);

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
};

#endif
