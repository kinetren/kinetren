#ifndef CLIPFXCHAINDIALOG_H
#define CLIPFXCHAINDIALOG_H

#include <QDialog>
#include <QJsonArray>
#include <QSize>
#include <QString>

class QLabel;
class QComboBox;
class QScrollArea;
class QWidget;
class QHBoxLayout;
class TimelineEditor;

/// Edit per-clip video or audio FX chain: horizontal chain + DnD reorder + widget parameters.
class ClipFxChainDialog : public QDialog
{
public:
    enum class MediaKind {
        Video,
        Audio
    };

    ClipFxChainDialog(QWidget *parent,
                       TimelineEditor *timeline,
                       int clipId,
                       MediaKind kind,
                       const QString &focusHint,
                       const QSize &projectOutputSize);

private:
    void onChainCardClicked(int index);
    void onChainReorder(int fromIndex, int toIndex);
    void rebuildChainStrip();
    void rebuildParamPanel();
    void applyParamsFromPanelToChain();
    void refreshPreview();
    void appendEffect(const QString &typeId);
    void removeSelectedEffect();
    QString effectTypeAt(int row) const;

    TimelineEditor *timeline = nullptr;
    int clipId = -1;
    MediaKind kind = MediaKind::Video;
    QSize projectOutputSize;
    QJsonArray workingChain;

    int selectedFxRow = -1;

    QScrollArea *chainScroll = nullptr;
    QWidget *chainInner = nullptr;
    QHBoxLayout *chainLayout = nullptr;

    QWidget *paramPanel = nullptr;
    QLabel *preview = nullptr;
    QComboBox *addCombo = nullptr;

    int suppressParamRefresh = 0;
};

#endif
