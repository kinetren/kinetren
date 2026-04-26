#ifndef CLIPFXCHAINDIALOG_H
#define CLIPFXCHAINDIALOG_H

#include <QDialog>
#include <QJsonArray>
#include <QSize>
#include <QString>

class QListWidget;
class QPlainTextEdit;
class QLabel;
class QComboBox;
class TimelineEditor;

/// Edit per-clip video or audio FX chain (JSON slots), with live video preview when editing video.
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
    void repopulateList();
    void loadParamsEditor();
    void saveParamsFromEditor();
    void refreshPreview();
    void appendEffect(const QString &typeId);

    TimelineEditor *timeline = nullptr;
    int clipId = -1;
    MediaKind kind = MediaKind::Video;
    QSize projectOutputSize;
    QJsonArray workingChain;

    QListWidget *fxList = nullptr;
    QPlainTextEdit *paramEdit = nullptr;
    QLabel *preview = nullptr;
    QComboBox *addCombo = nullptr;
    int suppressParamSignal = 0;
};

#endif
