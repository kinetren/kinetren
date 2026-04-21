#include "sourceeditwidgets.h"
#include "timelineeditor.h"

#include <cmath>

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

MorpherEditWindow::MorpherEditWindow(TimelineEditor *timeline, int morpherEventId, QWidget *parent)
    : QWidget(parent)
    , timeline(timeline)
    , morpherId(morpherEventId)
{
    setObjectName(QStringLiteral("MorpherEditWindow"));
    setWindowFlags(Qt::Window | Qt::WindowCloseButtonHint | Qt::WindowMinimizeButtonHint |
                   Qt::WindowMaximizeButtonHint);
    setAttribute(Qt::WA_DeleteOnClose, true);
    setMinimumSize(440, 300);
    resize(520, 360);
    setWindowTitle(tr("Morpher — event %1").arg(morpherId));
    setAcceptDrops(true);

    setStyleSheet(
        QStringLiteral(
            "#MorpherEditWindow { background: #0d1117; color: #e6edf3; }\n"
            "QLabel { color: #c9d1d9; font-size: 13px; }\n"
            "QLabel#MorpherSectionTitle { color: #f0f3f6; font-size: 12px; font-weight: 600; "
            "  padding-bottom: 4px; }\n"
            "QLabel#MorpherTargetsLabel { background: #161b22; border: 1px solid #30363d; border-radius: 8px; "
            "  padding: 12px; color: #8b949e; font-family: monospace; font-size: 12px; }\n"
            "QLineEdit, QDoubleSpinBox {\n"
            "  background: #161b22; color: #e6edf3; border: 1px solid #30363d; border-radius: 8px;\n"
            "  padding: 9px 12px; min-height: 22px; font-size: 13px;\n"
            "  selection-background-color: #1f6feb;\n"
            "}\n"
            "QLineEdit:focus, QDoubleSpinBox:focus { border: 1px solid #58a6ff; }\n"
            "QPushButton {\n"
            "  background: #21262d; color: #e6edf3; border: 1px solid #30363d; border-radius: 8px;\n"
            "  padding: 9px 18px; min-height: 22px; font-size: 13px;\n"
            "}\n"
            "QPushButton:hover { background: #30363d; border-color: #484f58; }\n"
            "QPushButton:pressed { background: #161b22; }\n"));

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(20, 18, 20, 18);
    layout->setSpacing(12);

    auto *targetsCaption = new QLabel(tr("Frozen targets"), this);
    targetsCaption->setObjectName(QStringLiteral("MorpherSectionTitle"));
    targetsLabel = new QLabel(this);
    targetsLabel->setObjectName(QStringLiteral("MorpherTargetsLabel"));
    targetsLabel->setWordWrap(true);
    targetsLabel->setTextInteractionFlags(Qt::TextSelectableByMouse);

    auto *repTitle = new QLabel(tr("Replace source"), this);
    repTitle->setObjectName(QStringLiteral("MorpherSectionTitle"));
    pathEdit = new QLineEdit(this);
    pathEdit->setPlaceholderText(tr("Path or drop media here"));
    browseButton = new QPushButton(tr("Browse…"), this);
    applyReplaceButton = new QPushButton(tr("Apply replace"), this);
    auto *repRow = new QHBoxLayout();
    repRow->setSpacing(10);
    repRow->addWidget(pathEdit, 1);
    repRow->addWidget(browseButton);

    auto *offTitle = new QLabel(tr("Source-in shift"), this);
    offTitle->setObjectName(QStringLiteral("MorpherSectionTitle"));
    deltaSpin = new QDoubleSpinBox(this);
    deltaSpin->setRange(-1.0e12, 1.0e12);
    deltaSpin->setDecimals(3);
    deltaSpin->setSuffix(tr(" ms"));
    applyOffsetButton = new QPushButton(tr("Apply shift"), this);

    layout->addWidget(targetsCaption);
    layout->addWidget(targetsLabel);
    layout->addSpacing(4);
    layout->addWidget(repTitle);
    layout->addLayout(repRow);
    layout->addWidget(applyReplaceButton);
    layout->addSpacing(4);
    layout->addWidget(offTitle);
    layout->addWidget(deltaSpin);
    layout->addWidget(applyOffsetButton);
    layout->addStretch(1);

    connect(browseButton, &QPushButton::clicked, this, &MorpherEditWindow::onBrowse);
    connect(applyReplaceButton, &QPushButton::clicked, this, &MorpherEditWindow::onApplyReplace);
    connect(applyOffsetButton, &QPushButton::clicked, this, &MorpherEditWindow::onApplyOffset);
    connect(pathEdit, &QLineEdit::returnPressed, this, &MorpherEditWindow::onApplyReplace);

    if (timeline) {
        connect(timeline, &TimelineEditor::timelineContentChanged, this, &MorpherEditWindow::reloadFromTimeline);
    }
    reloadFromTimeline();
}

void MorpherEditWindow::reloadFromTimeline()
{
    if (!timeline || !timeline->hasMorpherEvent(morpherId)) {
        close();
        return;
    }
    targetsLabel->setText(timeline->morpherTargetsDescription(morpherId));
    pathEdit->setText(timeline->morpherReplacePath(morpherId));
    deltaSpin->setValue(static_cast<double>(timeline->morpherSourceInDeltaMs(morpherId)));
}

void MorpherEditWindow::onBrowse()
{
    const QString path = QFileDialog::getOpenFileName(this, tr("Replace source"), pathEdit->text(),
                                                      tr("Media files") + QStringLiteral(" (*.*)"));
    if (!path.isEmpty()) {
        pathEdit->setText(path);
    }
}

void MorpherEditWindow::applyPath(const QString &path)
{
    if (!timeline || path.isEmpty()) {
        return;
    }
    timeline->setMorpherReplacePath(morpherId, path);
}

void MorpherEditWindow::onApplyReplace()
{
    applyPath(pathEdit->text().trimmed());
}

void MorpherEditWindow::onApplyOffset()
{
    if (!timeline) {
        return;
    }
    const qint64 v = static_cast<qint64>(std::llround(deltaSpin->value()));
    timeline->setMorpherAbsoluteSourceInMs(morpherId, -1);
    timeline->setMorpherSourceInDeltaMs(morpherId, v);
}

void MorpherEditWindow::dragEnterEvent(QDragEnterEvent *event)
{
    if (event->mimeData()->hasUrls() || event->mimeData()->hasText()) {
        event->acceptProposedAction();
        return;
    }
    QWidget::dragEnterEvent(event);
}

void MorpherEditWindow::dropEvent(QDropEvent *event)
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
    if (targetMorpherId >= 0) {
        timeline->setMorpherReplacePath(targetMorpherId, path);
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
    if (targetMorpherId >= 0) {
        timeline->setMorpherAbsoluteSourceInMs(targetMorpherId, -1);
        timeline->setMorpherSourceInDeltaMs(targetMorpherId, v);
        return;
    }
    if (kind == TargetKind::Clip) {
        timeline->applyAbsoluteSourceStartToClip(targetClipId, v);
    } else if (kind == TargetKind::RegionShift) {
        timeline->applyRegionSourceShift(v, regionAllTracks, regionTrackIndex);
    } else if (kind == TargetKind::Morpher) {
        timeline->setMorpherAbsoluteSourceInMs(targetMorpherId, -1);
        timeline->setMorpherSourceInDeltaMs(targetMorpherId, v);
    }
}
