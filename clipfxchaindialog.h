#ifndef CLIPFXCHAINDIALOG_H
#define CLIPFXCHAINDIALOG_H

#include <QJsonArray>
#include <QSize>
#include <QString>
#include <QWidget>

class AudioPreviewWidget;
class FxChainEditorWidget;
class QLabel;
class TimelineEditor;

/// Non-modal per-clip video/audio FX editor: live updates on the timeline; Cancel restores the chain from when the window was opened.
class ClipFxChainWindow : public QWidget
{
    Q_OBJECT
public:
    enum class MediaKind {
        Video,
        Audio
    };

    ClipFxChainWindow(QWidget *parent,
                      TimelineEditor *timeline,
                      int clipId,
                      MediaKind kind,
                      const QString &focusHint,
                      const QSize &projectOutputSize);

    qint64 stableKey() const;

private slots:
    void onEditorChainChanged(const QJsonArray &chain);
    void onDoneClicked();
    void onCancelClicked();

private:
    void refreshPreview();
    void refreshAudioPreview();
    void pushChainToTimeline(const QJsonArray &chain);

    TimelineEditor *timeline = nullptr;
    int clipId = -1;
    MediaKind kind = MediaKind::Video;
    QSize projectOutputSize;
    QJsonArray backupChain;

    FxChainEditorWidget *editor = nullptr;
    QLabel *videoPreview = nullptr;
    AudioPreviewWidget *audioPreview = nullptr;
};

#endif
