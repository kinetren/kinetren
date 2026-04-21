#include "sourceeditwidgets.h"
#include "timelineeditor.h"

#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QHBoxLayout>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMimeData>
#include <QPushButton>
#include <QVBoxLayout>

SourceReplacePanel::SourceReplacePanel(TimelineEditor *timeline, QWidget *parent)
    : QWidget(parent)
    , timeline(timeline)
{
    auto *layout = new QVBoxLayout(this);
    titleLabel = new QLabel(tr("Replace source"), this);
    pathEdit = new QLineEdit(this);
    pathEdit->setPlaceholderText(tr("Media file path (or drop here)"));
    browseButton = new QPushButton(tr("Browse…"), this);
    applyButton = new QPushButton(tr("Apply"), this);
    auto *row = new QHBoxLayout();
    row->addWidget(pathEdit, 1);
    row->addWidget(browseButton);
    layout->addWidget(titleLabel);
    layout->addLayout(row);
    layout->addWidget(applyButton);

    setAcceptDrops(true);

    connect(browseButton, &QPushButton::clicked, this, &SourceReplacePanel::onBrowse);
    connect(applyButton, &QPushButton::clicked, this, &SourceReplacePanel::onApply);
    connect(pathEdit, &QLineEdit::returnPressed, this, &SourceReplacePanel::onApply);

    clearTarget();
}

void SourceReplacePanel::setClipTarget(int clipId)
{
    kind = TargetKind::Clip;
    targetClipId = clipId;
    regionAllTracks = false;
    regionTrackIndex = -1;
    targetMorpherId = -1;
    refreshUi();
}

void SourceReplacePanel::setRegionTarget(bool allTracks, int trackIndex)
{
    kind = TargetKind::Region;
    targetClipId = -1;
    regionAllTracks = allTracks;
    regionTrackIndex = trackIndex;
    targetMorpherId = -1;
    refreshUi();
}

void SourceReplacePanel::setMorpherTarget(int id)
{
    kind = TargetKind::Morpher;
    targetClipId = -1;
    targetMorpherId = id;
    refreshUi();
}

void SourceReplacePanel::clearTarget()
{
    kind = TargetKind::None;
    targetClipId = -1;
    targetMorpherId = -1;
    refreshUi();
}

void SourceReplacePanel::refreshUi()
{
    QString t = tr("Replace source — no target");
    if (kind == TargetKind::Clip && timeline) {
        t = tr("Replace source — clip %1").arg(targetClipId);
        pathEdit->setText(timeline->clipSourcePath(targetClipId));
    } else if (kind == TargetKind::Region) {
        t = regionAllTracks ? tr("Replace source — region (all tracks)") : tr("Replace source — region (this track)");
        pathEdit->clear();
    } else if (kind == TargetKind::Morpher && timeline) {
        t = tr("Replace source — morpher %1").arg(targetMorpherId);
        pathEdit->setText(timeline->morpherReplacePath(targetMorpherId));
    } else {
        pathEdit->clear();
    }
    titleLabel->setText(t);
    const bool on = kind != TargetKind::None;
    pathEdit->setEnabled(on);
    browseButton->setEnabled(on);
    applyButton->setEnabled(on);
}

void SourceReplacePanel::onBrowse()
{
    const QString path = QFileDialog::getOpenFileName(this, tr("Replace source"), pathEdit->text(),
                                                      tr("Media files") + QStringLiteral(" (*.*)"));
    if (!path.isEmpty()) {
        pathEdit->setText(path);
    }
}

void SourceReplacePanel::applyPath(const QString &path)
{
    if (!timeline || path.isEmpty()) {
        return;
    }
    if (kind == TargetKind::Clip) {
        timeline->applyReplaceSourceToClip(targetClipId, path);
    } else if (kind == TargetKind::Region) {
        timeline->applyReplaceSourceToRegion(path, regionAllTracks, regionTrackIndex);
    } else if (kind == TargetKind::Morpher) {
        timeline->setMorpherReplacePath(targetMorpherId, path);
    }
}

void SourceReplacePanel::onApply()
{
    applyPath(pathEdit->text().trimmed());
}

void SourceReplacePanel::dragEnterEvent(QDragEnterEvent *event)
{
    if (event->mimeData()->hasUrls() || event->mimeData()->hasText()) {
        event->acceptProposedAction();
        return;
    }
    QWidget::dragEnterEvent(event);
}

void SourceReplacePanel::dropEvent(QDropEvent *event)
{
    const QString p = TimelineEditor::firstLocalPathFromMime(event->mimeData());
    if (!p.isEmpty()) {
        pathEdit->setText(p);
        applyPath(p);
        event->acceptProposedAction();
        return;
    }
    QWidget::dropEvent(event);
}

SourceOffsetPanel::SourceOffsetPanel(TimelineEditor *timeline, QWidget *parent)
    : QWidget(parent)
    , timeline(timeline)
{
    auto *layout = new QVBoxLayout(this);
    titleLabel = new QLabel(tr("Source offset"), this);
    valueSpin = new QDoubleSpinBox(this);
    valueSpin->setRange(-1.0e12, 1.0e12);
    valueSpin->setDecimals(3);
    valueSpin->setSuffix(tr(" ms"));
    applyButton = new QPushButton(tr("Apply"), this);
    layout->addWidget(titleLabel);
    layout->addWidget(valueSpin);
    layout->addWidget(applyButton);
    connect(applyButton, &QPushButton::clicked, this, &SourceOffsetPanel::onApply);
    clearTarget();
}

void SourceOffsetPanel::setClipTarget(int clipId)
{
    kind = TargetKind::Clip;
    targetClipId = clipId;
    regionAllTracks = false;
    regionTrackIndex = -1;
    targetMorpherId = -1;
    refreshUi();
}

void SourceOffsetPanel::setRegionShiftTarget(bool allTracks, int trackIndex)
{
    kind = TargetKind::RegionShift;
    targetClipId = -1;
    regionAllTracks = allTracks;
    regionTrackIndex = trackIndex;
    targetMorpherId = -1;
    refreshUi();
}

void SourceOffsetPanel::setMorpherTarget(int id)
{
    kind = TargetKind::Morpher;
    targetClipId = -1;
    targetMorpherId = id;
    refreshUi();
}

void SourceOffsetPanel::clearTarget()
{
    kind = TargetKind::None;
    targetClipId = -1;
    targetMorpherId = -1;
    refreshUi();
}

void SourceOffsetPanel::refreshUi()
{
    QString t = tr("Source offset — no target");
    if (kind == TargetKind::Clip && timeline) {
        t = tr("Source start (absolute) — clip %1").arg(targetClipId);
        valueSpin->setValue(static_cast<double>(timeline->clipSourceInMs(targetClipId)));
    } else if (kind == TargetKind::RegionShift) {
        t = regionAllTracks ? tr("Shift source start — region (all tracks)") : tr("Shift source start — region (this track)");
        valueSpin->setValue(0.0);
    } else if (kind == TargetKind::Morpher && timeline) {
        t = tr("Source in delta — morpher %1").arg(targetMorpherId);
        valueSpin->setValue(static_cast<double>(timeline->morpherSourceInDeltaMs(targetMorpherId)));
    } else {
        valueSpin->setValue(0.0);
    }
    titleLabel->setText(t);
    const bool on = kind != TargetKind::None;
    valueSpin->setEnabled(on);
    applyButton->setEnabled(on);
}

void SourceOffsetPanel::onApply()
{
    if (!timeline) {
        return;
    }
    const qint64 v = static_cast<qint64>(std::llround(valueSpin->value()));
    if (kind == TargetKind::Clip) {
        timeline->applyAbsoluteSourceStartToClip(targetClipId, v);
    } else if (kind == TargetKind::RegionShift) {
        timeline->applyRegionSourceShift(v, regionAllTracks, regionTrackIndex);
    } else if (kind == TargetKind::Morpher) {
        timeline->setMorpherAbsoluteSourceInMs(targetMorpherId, -1);
        timeline->setMorpherSourceInDeltaMs(targetMorpherId, v);
    }
}
