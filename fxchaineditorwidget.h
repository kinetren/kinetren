#ifndef FXCHAINEDITORWIDGET_H
#define FXCHAINEDITORWIDGET_H

#include <QJsonArray>
#include <QWidget>

class TimelineEditor;

/// Horizontal FX chain + parameter widgets (shared by clip FX window and morpher FX batch).
class FxChainEditorWidget : public QWidget
{
    Q_OBJECT
public:
    enum class MediaKind {
        Video,
        Audio
    };

    explicit FxChainEditorWidget(QWidget *parent = nullptr);

    void setMediaKind(MediaKind k);
    /// Enables "Auto from clip" for camera motion when non-null.
    void setCamMotionEstimateContext(TimelineEditor *timeline, int clipId);
    void setChain(const QJsonArray &chain);
    /// Select a chain tile without emitting chainChanged (UI only).
    void selectRow(int row);
    QJsonArray chain() const { return workingChain; }

signals:
    /// Emitted after any edit (reorder, add/remove, parameter tweak).
    void chainChanged(const QJsonArray &chain);

private slots:
    void onChainCardClicked(int index);
    void onChainReorder(int fromIndex, int toIndex);
    void onAddClicked();
    void onRemoveClicked();

private:
    QString effectTypeAt(int row) const;
    void rebuildChainStrip();
    void rebuildParamPanel();
    void applyParamsFromPanelToChain();

    MediaKind mediaKind = MediaKind::Video;
    QJsonArray workingChain;
    int selectedFxRow = -1;
    int suppressParamRefresh = 0;

    class QScrollArea *chainScroll = nullptr;
    QWidget *chainInner = nullptr;
    class QHBoxLayout *chainLayout = nullptr;
    QWidget *paramPanel = nullptr;
    class QComboBox *addCombo = nullptr;

    TimelineEditor *camMotionTimeline = nullptr;
    int camMotionClipId = -1;
};

#endif
