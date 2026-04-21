#include "sourceeditwidgets.h"
#include "timelineeditor.h"

#include <algorithm>
#include <cmath>

#include <QApplication>
#include <QCoreApplication>
#include <QDialog>
#include <QDir>
#include <QDoubleSpinBox>
#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QEvent>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMimeData>
#include <QPushButton>
#include <QSignalBlocker>
#include <QStyle>
#include <QUrl>
#include <QVBoxLayout>

namespace {

QString pickMediaFile(QWidget *parent, const QString &title, const QString &startPath)
{
    QWidget *dlgParent = parent ? parent->window() : nullptr;
    QFileDialog dlg(dlgParent, title);
    dlg.setNameFilter(QCoreApplication::translate("SourceEditWidgets", "Media files") +
                      QStringLiteral(" (*.*)"));
    dlg.setFileMode(QFileDialog::ExistingFile);
    dlg.setAcceptMode(QFileDialog::AcceptOpen);
    dlg.setOption(QFileDialog::DontUseNativeDialog, true);
    dlg.setWindowModality(Qt::WindowModal);

    const QString trimmed = startPath.trimmed();
    const QFileInfo fi(trimmed);
    if (fi.exists() && fi.isFile()) {
        dlg.setDirectory(fi.absolutePath());
        dlg.selectFile(fi.fileName());
    } else if (fi.isAbsolute()) {
        const QDir dir(fi.absolutePath());
        if (dir.exists()) {
            dlg.setDirectory(dir.absolutePath());
        }
    }

    if (dlg.exec() == QDialog::Accepted) {
        const QStringList files = dlg.selectedFiles();
        if (!files.isEmpty()) {
            return files.first();
        }
    }
    return {};
}

bool mimeLooksLikePath(const QMimeData *mime)
{
    return mime && (mime->hasUrls() || mime->hasText());
}

} // namespace

MorpherEditWindow::MorpherEditWindow(TimelineEditor *timeline, int morpherEventId, QWidget *parent)
    : QWidget(parent)
    , timeline(timeline)
    , morpherId(morpherEventId)
{
    setObjectName(QStringLiteral("MorpherEditWindow"));
    setWindowFlags(Qt::Window | Qt::WindowCloseButtonHint | Qt::WindowMinimizeButtonHint |
                   Qt::WindowMaximizeButtonHint);
    setAttribute(Qt::WA_DeleteOnClose, true);
    setMinimumSize(410, 275);
    resize(500, 330);
    setWindowTitle(tr("Morpher — event %1").arg(morpherId));
    setAcceptDrops(true);

    // Match the main window font; avoid font-size in QSS (fixed px fights HiDPI / 4K scaling).
    if (QWidget *pw = parentWidget()) {
        setFont(pw->font());
    } else {
        setFont(QApplication::font());
    }

    setStyleSheet(
        QStringLiteral(
            "#MorpherEditWindow { background: #0d1117; color: #e6edf3; }\n"
            "QLabel { color: #c9d1d9; }\n"
            "QLabel#MorpherSectionTitle { color: #f0f3f6; font-weight: 600; padding-bottom: 2px; }\n"
            "QLabel#MorpherTargetsLabel { background: #161b22; border: 1px solid #30363d; border-radius: 6px; "
            "  padding: 8px; color: #8b949e; }\n"
            "QLineEdit, QDoubleSpinBox {\n"
            "  background: #161b22; color: #e6edf3; border: 1px solid #30363d; border-radius: 6px;\n"
            "  padding: 6px 8px;\n"
            "  selection-background-color: #1f6feb;\n"
            "}\n"
            "QLineEdit:focus, QDoubleSpinBox:focus { border: 1px solid #58a6ff; }\n"
            "QPushButton {\n"
            "  background: #21262d; color: #e6edf3; border: 1px solid #30363d; border-radius: 6px;\n"
            "  padding: 6px 14px;\n"
            "}\n"
            "QPushButton:hover { background: #30363d; border-color: #484f58; }\n"
            "QPushButton:pressed { background: #161b22; }\n"));

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(12, 10, 12, 10);
    layout->setSpacing(8);

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
    pathEdit->setAcceptDrops(true);
    pathEdit->installEventFilter(this);
    browseButton = new QPushButton(this);
    browseButton->setToolTip(tr("Browse…"));
    browseButton->setIcon(style()->standardIcon(QStyle::SP_DialogOpenButton));
    {
        const int side = std::max(32, fontMetrics().height() + 14);
        browseButton->setIconSize(QSize(side - 10, side - 10));
        browseButton->setFixedSize(side, side);
    }
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

    layout->addWidget(targetsCaption);
    layout->addWidget(targetsLabel);
    layout->addSpacing(4);
    layout->addWidget(repTitle);
    layout->addLayout(repRow);
    layout->addSpacing(4);
    layout->addWidget(offTitle);
    layout->addWidget(deltaSpin);
    layout->addStretch(1);

    connect(browseButton, &QPushButton::clicked, this, &MorpherEditWindow::onBrowse);
    connect(pathEdit, &QLineEdit::textChanged, this, &MorpherEditWindow::onPathEditChanged);
    connect(deltaSpin, qOverload<double>(&QDoubleSpinBox::valueChanged), this,
            &MorpherEditWindow::onDeltaSpinChanged);

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

    const QString newPath = timeline->morpherReplacePath(morpherId);
    if (pathEdit->text() != newPath) {
        QSignalBlocker block(pathEdit);
        pathEdit->setText(newPath);
    }

    const double newDelta = static_cast<double>(timeline->morpherSourceInDeltaMs(morpherId));
    if (deltaSpin->value() != newDelta) {
        QSignalBlocker block(deltaSpin);
        deltaSpin->setValue(newDelta);
    }
}

void MorpherEditWindow::onBrowse()
{
    const QString path = pickMediaFile(this, tr("Replace source"), pathEdit->text());
    if (!path.isEmpty()) {
        pathEdit->setText(path);
    }
}

void MorpherEditWindow::applyPath(const QString &path)
{
    if (!timeline) {
        return;
    }
    timeline->setMorpherReplacePath(morpherId, path.trimmed());
}

void MorpherEditWindow::onPathEditChanged(const QString &text)
{
    applyPath(text);
}

void MorpherEditWindow::onDeltaSpinChanged(double value)
{
    if (!timeline) {
        return;
    }
    const qint64 v = static_cast<qint64>(std::llround(value));
    timeline->setMorpherAbsoluteSourceInMs(morpherId, -1);
    timeline->setMorpherSourceInDeltaMs(morpherId, v);
}

void MorpherEditWindow::dragEnterEvent(QDragEnterEvent *event)
{
    if (mimeLooksLikePath(event->mimeData())) {
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

bool MorpherEditWindow::eventFilter(QObject *watched, QEvent *event)
{
    if (watched != pathEdit) {
        return QWidget::eventFilter(watched, event);
    }
    if (event->type() == QEvent::DragEnter) {
        auto *e = static_cast<QDragEnterEvent *>(event);
        if (mimeLooksLikePath(e->mimeData())) {
            e->acceptProposedAction();
            return true;
        }
    } else if (event->type() == QEvent::DragMove) {
        auto *e = static_cast<QDragMoveEvent *>(event);
        if (mimeLooksLikePath(e->mimeData())) {
            e->acceptProposedAction();
            return true;
        }
    } else if (event->type() == QEvent::Drop) {
        auto *e = static_cast<QDropEvent *>(event);
        const QString p = TimelineEditor::firstLocalPathFromMime(e->mimeData());
        if (!p.isEmpty()) {
            pathEdit->setText(p);
            applyPath(p);
            e->acceptProposedAction();
            return true;
        }
    }
    return QWidget::eventFilter(watched, event);
}

SourceReplacePanel::SourceReplacePanel(TimelineEditor *timeline, QWidget *parent)
    : QWidget(parent)
    , timeline(timeline)
{
    auto *layout = new QVBoxLayout(this);
    titleLabel = new QLabel(tr("Replace source"), this);
    pathEdit = new QLineEdit(this);
    pathEdit->setPlaceholderText(tr("Media file path (or drop here)"));
    pathEdit->setAcceptDrops(true);
    pathEdit->installEventFilter(this);
    browseButton = new QPushButton(this);
    browseButton->setToolTip(tr("Browse…"));
    browseButton->setIcon(style()->standardIcon(QStyle::SP_DialogOpenButton));
    {
        const int side = std::max(32, fontMetrics().height() + 14);
        browseButton->setIconSize(QSize(side - 10, side - 10));
        browseButton->setFixedSize(side, side);
    }
    auto *row = new QHBoxLayout();
    row->addWidget(pathEdit, 1);
    row->addWidget(browseButton);
    layout->addWidget(titleLabel);
    layout->addLayout(row);

    setAcceptDrops(true);

    connect(browseButton, &QPushButton::clicked, this, &SourceReplacePanel::onBrowse);
    connect(pathEdit, &QLineEdit::textChanged, this, &SourceReplacePanel::onPathEditChanged);

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
    {
        QSignalBlocker block(pathEdit);
        if (kind == TargetKind::Clip && timeline) {
            t = tr("Replace source — clip %1").arg(targetClipId);
            pathEdit->setText(timeline->clipSourcePath(targetClipId));
        } else if (kind == TargetKind::Region) {
            t = regionAllTracks ? tr("Replace source — region (all tracks)")
                                : tr("Replace source — region (this track)");
            pathEdit->clear();
        } else if (kind == TargetKind::Morpher && timeline) {
            t = tr("Replace source — morpher %1").arg(targetMorpherId);
            pathEdit->setText(timeline->morpherReplacePath(targetMorpherId));
        } else {
            pathEdit->clear();
        }
    }
    titleLabel->setText(t);
    const bool on = kind != TargetKind::None;
    pathEdit->setEnabled(on);
    browseButton->setEnabled(on);
}

void SourceReplacePanel::onBrowse()
{
    const QString path = pickMediaFile(this, tr("Replace source"), pathEdit->text());
    if (!path.isEmpty()) {
        pathEdit->setText(path);
    }
}

void SourceReplacePanel::applyPath(const QString &path)
{
    if (!timeline) {
        return;
    }
    const QString t = path.trimmed();
    if (t.isEmpty()) {
        if (kind == TargetKind::Morpher && targetMorpherId >= 0) {
            timeline->setMorpherReplacePath(targetMorpherId, QString());
        }
        return;
    }
    if (targetMorpherId >= 0) {
        timeline->setMorpherReplacePath(targetMorpherId, t);
        return;
    }
    if (kind == TargetKind::Clip) {
        timeline->applyReplaceSourceToClip(targetClipId, t);
    } else if (kind == TargetKind::Region) {
        timeline->applyReplaceSourceToRegion(t, regionAllTracks, regionTrackIndex);
    } else if (kind == TargetKind::Morpher) {
        timeline->setMorpherReplacePath(targetMorpherId, t);
    }
}

void SourceReplacePanel::onPathEditChanged(const QString &text)
{
    applyPath(text);
}

void SourceReplacePanel::dragEnterEvent(QDragEnterEvent *event)
{
    if (mimeLooksLikePath(event->mimeData())) {
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

bool SourceReplacePanel::eventFilter(QObject *watched, QEvent *event)
{
    if (watched != pathEdit) {
        return QWidget::eventFilter(watched, event);
    }
    if (event->type() == QEvent::DragEnter) {
        auto *e = static_cast<QDragEnterEvent *>(event);
        if (mimeLooksLikePath(e->mimeData())) {
            e->acceptProposedAction();
            return true;
        }
    } else if (event->type() == QEvent::DragMove) {
        auto *e = static_cast<QDragMoveEvent *>(event);
        if (mimeLooksLikePath(e->mimeData())) {
            e->acceptProposedAction();
            return true;
        }
    } else if (event->type() == QEvent::Drop) {
        auto *e = static_cast<QDropEvent *>(event);
        const QString p = TimelineEditor::firstLocalPathFromMime(e->mimeData());
        if (!p.isEmpty()) {
            pathEdit->setText(p);
            applyPath(p);
            e->acceptProposedAction();
            return true;
        }
    }
    return QWidget::eventFilter(watched, event);
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
    layout->addWidget(titleLabel);
    layout->addWidget(valueSpin);
    connect(valueSpin, qOverload<double>(&QDoubleSpinBox::valueChanged), this,
            &SourceOffsetPanel::onOffsetSpinChanged);
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
    {
        QSignalBlocker block(valueSpin);
        if (kind == TargetKind::Clip && timeline) {
            t = tr("Source start (absolute) — clip %1").arg(targetClipId);
            valueSpin->setValue(static_cast<double>(timeline->clipSourceInMs(targetClipId)));
        } else if (kind == TargetKind::RegionShift) {
            t = regionAllTracks ? tr("Shift source start — region (all tracks)")
                                : tr("Shift source start — region (this track)");
            valueSpin->setValue(0.0);
        } else if (kind == TargetKind::Morpher && timeline) {
            t = tr("Source in delta — morpher %1").arg(targetMorpherId);
            valueSpin->setValue(static_cast<double>(timeline->morpherSourceInDeltaMs(targetMorpherId)));
        } else {
            valueSpin->setValue(0.0);
        }
    }
    titleLabel->setText(t);
    const bool on = kind != TargetKind::None;
    valueSpin->setEnabled(on);
}

void SourceOffsetPanel::onOffsetSpinChanged(double value)
{
    if (!timeline) {
        return;
    }
    const qint64 v = static_cast<qint64>(std::llround(value));
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
