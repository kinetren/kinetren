#include "clipfxchaindialog.h"

#include "fxchaineditorwidget.h"
#include "timelineeditor.h"
#include "bc_fx.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QSplitter>
#include <QVBoxLayout>

ClipFxChainWindow::ClipFxChainWindow(QWidget *parent,
                                     TimelineEditor *timeline,
                                     int clipId,
                                     MediaKind kind,
                                     const QString &focusHint,
                                     const QSize &projectOutputSize)
    : QWidget(parent)
    , timeline(timeline)
    , clipId(clipId)
    , kind(kind)
    , projectOutputSize(projectOutputSize)
{
    setObjectName(QStringLiteral("ClipFxChainWindow"));
    setWindowFlags(Qt::Tool | Qt::Window | Qt::WindowCloseButtonHint | Qt::WindowMinimizeButtonHint |
                   Qt::WindowMaximizeButtonHint);
    setAttribute(Qt::WA_DeleteOnClose, true);
    setMinimumSize(760, 520);
    setWindowTitle(kind == MediaKind::Video ? tr("Video effects") : tr("Audio effects"));

    if (kind == MediaKind::Video) {
        backupChain = timeline->clipVideoFxChain(clipId);
    } else {
        backupChain = timeline->clipAudioFxChain(clipId);
    }

    QJsonArray initial = backupChain;
    if (kind == MediaKind::Video && initial.isEmpty()) {
        initial = burstcon::defaultVideoFxChain();
    }

    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(10, 10, 10, 10);
    root->setSpacing(8);

    auto *split = new QSplitter(Qt::Vertical, this);

    editor = new FxChainEditorWidget(split);
    editor->setMediaKind(kind == MediaKind::Video ? FxChainEditorWidget::MediaKind::Video
                                                   : FxChainEditorWidget::MediaKind::Audio);
    if (kind == MediaKind::Video && timeline) {
        editor->setCamMotionEstimateContext(timeline, clipId);
    }
    editor->setChain(initial);
    connect(editor, &FxChainEditorWidget::chainChanged, this, &ClipFxChainWindow::onEditorChainChanged);
    if (timeline) {
        connect(timeline, &TimelineEditor::timelineContentChanged, editor, &FxChainEditorWidget::refreshAutomationUi);
    }

    if (kind == MediaKind::Video) {
        preview = new QLabel(split);
        preview->setMinimumHeight(200);
        preview->setAlignment(Qt::AlignCenter);
        preview->setStyleSheet(QStringLiteral("QLabel { background:#101214; border:1px solid #343a42; }"));
        preview->setText(tr("Preview"));
        split->addWidget(editor);
        split->addWidget(preview);
        split->setStretchFactor(0, 3);
        split->setStretchFactor(1, 2);
    } else {
        split->addWidget(editor);
    }
    root->addWidget(split, 1);

    auto *btnRow = new QHBoxLayout();
    btnRow->addStretch(1);
    auto *cancelBtn = new QPushButton(tr("Cancel"), this);
    auto *doneBtn = new QPushButton(tr("Done"), this);
    cancelBtn->setDefault(false);
    doneBtn->setDefault(true);
    btnRow->addWidget(cancelBtn);
    btnRow->addWidget(doneBtn);
    root->addLayout(btnRow);

    connect(cancelBtn, &QPushButton::clicked, this, &ClipFxChainWindow::onCancelClicked);
    connect(doneBtn, &QPushButton::clicked, this, &ClipFxChainWindow::onDoneClicked);

    if (timeline) {
        timeline->pushUndoSnapshot();
    }
    pushChainToTimeline(initial);

    int selRow = -1;
    const QJsonArray wc = editor->chain();
    if (!wc.isEmpty()) {
        if (focusHint == QStringLiteral("pan")) {
            for (int i = 0; i < wc.size(); ++i) {
                if (wc.at(i).toObject().value(QStringLiteral("type")).toString() == QStringLiteral("pan")) {
                    selRow = i;
                    break;
                }
            }
        }
        if (selRow < 0) {
            selRow = wc.size() - 1;
        }
        editor->selectRow(selRow);
    }
    refreshPreview();
}

qint64 ClipFxChainWindow::stableKey() const
{
    return (static_cast<qint64>(clipId) << 1) | (kind == MediaKind::Video ? 0LL : 1LL);
}

void ClipFxChainWindow::pushChainToTimeline(const QJsonArray &chain)
{
    if (!timeline) {
        return;
    }
    if (kind == MediaKind::Video) {
        QJsonArray c = chain;
        if (c.isEmpty()) {
            c = burstcon::defaultVideoFxChain();
        }
        timeline->setClipVideoFxChain(clipId, c);
    } else {
        timeline->setClipAudioFxChain(clipId, chain, false);
    }
}

void ClipFxChainWindow::onEditorChainChanged(const QJsonArray &chain)
{
    pushChainToTimeline(chain);
    refreshPreview();
}

void ClipFxChainWindow::refreshPreview()
{
    if (kind != MediaKind::Video || !preview || !timeline) {
        return;
    }
    QImage img = timeline->previewStillForClip(clipId);
    if (img.isNull()) {
        preview->setText(tr("No preview frame"));
        return;
    }
    const int pw = std::max(2, std::min(640, projectOutputSize.width() > 0 ? projectOutputSize.width() : 1280));
    const int ph = std::max(2, std::min(360, projectOutputSize.height() > 0 ? projectOutputSize.height() : 720));
    QJsonArray chain = timeline->clipVideoFxChain(clipId);
    burstcon::VideoBurst vb;
    vb.quality = burstcon::BurstQuality{img.width(), img.height()};
    vb.frame = img.convertToFormat(QImage::Format_ARGB32);
    burstcon::BurstIndex clipDurBurst = 0;
    burstcon::BurstIndex decodeOrigin = -1;
    burstcon::BurstIndex sampleIndex = 0;
    if (timeline->clipMatchesFxTarget(clipId, true)) {
        const qint64 durationMs = timeline->clipDurationMs(clipId);
        const qint64 sourceInMs = timeline->clipSourceInMs(clipId);
        clipDurBurst = burstcon::msToBurstIndexCeil(durationMs);
        decodeOrigin = burstcon::msToBurstIndex(sourceInMs);
        const qint64 midOff = std::max<qint64>(0, durationMs / 2);
        sampleIndex = burstcon::msToBurstIndex(sourceInMs + midOff);
    }
    vb.index = sampleIndex;
    burstcon::VideoFxContext ctx{{pw, ph}, sampleIndex, {}, clipDurBurst, decodeOrigin};
    burstcon::processVideoFxChainInPlace(vb, chain, ctx);
    if (vb.frame.isNull()) {
        preview->setText(tr("Effect error"));
        return;
    }
    preview->setPixmap(QPixmap::fromImage(vb.frame.scaled(pw, ph, Qt::KeepAspectRatio, Qt::SmoothTransformation)));
}

void ClipFxChainWindow::onDoneClicked()
{
    close();
}

void ClipFxChainWindow::onCancelClicked()
{
    if (timeline) {
        if (kind == MediaKind::Video) {
            timeline->setClipVideoFxChain(clipId, backupChain);
        } else {
            timeline->setClipAudioFxChain(clipId, backupChain, false);
        }
    }
    close();
}
