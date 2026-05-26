#include "clipfxchaindialog.h"

#include "audiopreviewwidget.h"
#include "fxchaineditorwidget.h"
#include "timelineeditor.h"
#include "bc_fx.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QSplitter>
#include <QVBoxLayout>

#include <algorithm>

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
        connect(timeline, &TimelineEditor::timelineContentChanged, this, [this]() {
            refreshAudioPreview();
            if (this->kind == MediaKind::Video) {
                refreshPreview();
            }
        });
    }

    split->addWidget(editor);
    split->setStretchFactor(0, 3);

    auto *previewRow = new QWidget(split);
    auto *previewRowLayout = new QHBoxLayout(previewRow);
    previewRowLayout->setContentsMargins(0, 0, 0, 0);
    previewRowLayout->setSpacing(6);

    if (kind == MediaKind::Video) {
        videoPreview = new QLabel(previewRow);
        videoPreview->setMinimumHeight(200);
        videoPreview->setAlignment(Qt::AlignCenter);
        videoPreview->setStyleSheet(QStringLiteral("QLabel { background:#101214; border:1px solid #343a42; }"));
        videoPreview->setText(tr("Preview"));
        previewRowLayout->addWidget(videoPreview, 1);
    }

    audioPreview = new AudioPreviewWidget(previewRow);
    audioPreview->setMinimumHeight(kind == MediaKind::Video ? 200 : 120);
    previewRowLayout->addWidget(audioPreview);

    split->addWidget(previewRow);
    split->setStretchFactor(1, 2);
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
    refreshAudioPreview();
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
    refreshAudioPreview();
}

void ClipFxChainWindow::refreshPreview()
{
    if (kind != MediaKind::Video || !videoPreview || !timeline) {
        return;
    }
    QImage img = timeline->previewStillForClip(clipId);
    if (img.isNull()) {
        videoPreview->setText(tr("No preview frame"));
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
        videoPreview->setText(tr("Effect error"));
        return;
    }
    videoPreview->setPixmap(
        QPixmap::fromImage(vb.frame.scaled(pw, ph, Qt::KeepAspectRatio, Qt::SmoothTransformation)));
}

void ClipFxChainWindow::refreshAudioPreview()
{
    if (!audioPreview || !timeline) {
        return;
    }
    int peaksClipId = clipId;
    QJsonArray fxChain;
    if (kind == MediaKind::Audio) {
        fxChain = timeline->clipAudioFxChain(clipId);
    } else {
        const int linkedId = timeline->linkedAudioClipId(clipId);
        if (linkedId > 0) {
            peaksClipId = linkedId;
            fxChain = timeline->clipAudioFxChain(linkedId);
        }
    }
    if (kind == MediaKind::Video && timeline->linkedAudioClipId(clipId) < 0) {
        peaksClipId = clipId;
        fxChain = QJsonArray();
    }
    QVector<QPair<int, int>> peaks;
    if (kind == MediaKind::Audio && !fxChain.isEmpty()) {
        peaks = timeline->clipAudioPeaksWithFxPreview(peaksClipId, fxChain, 128);
    } else {
        peaks = timeline->clipAudioPeaksForPreview(peaksClipId, 128);
    }
    if (peaks.isEmpty()) {
        const bool videoEmbedded =
            kind == MediaKind::Video && timeline->linkedAudioClipId(clipId) < 0;
        if (videoEmbedded) {
            audioPreview->setStatusText(tr("Loading waveform…"));
        } else if (kind == MediaKind::Video) {
            audioPreview->setStatusText(tr("No linked audio"));
        } else {
            audioPreview->setStatusText(tr("Loading waveform…"));
        }
        return;
    }
    double playRel = -1.0;
    if (timeline->clipMatchesFxTarget(peaksClipId, false) ||
        (kind == MediaKind::Video && peaksClipId == clipId && timeline->clipMatchesFxTarget(clipId, true))) {
        const qint64 dur = std::max<qint64>(1, timeline->clipDurationMs(peaksClipId));
        const qint64 midOff = std::max<qint64>(0, dur / 2);
        playRel = static_cast<double>(midOff) / static_cast<double>(dur - 1);
    }
    audioPreview->setPeaks(peaks);
    audioPreview->setPlayheadRel(playRel);
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
