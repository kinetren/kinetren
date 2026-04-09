#include "sourceareawidget.h"

#include <QDrag>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFileInfo>
#include <QMimeData>
#include <QUrl>

SourceAreaWidget::SourceAreaWidget(QWidget *parent)
    : QListWidget(parent)
{
    setAcceptDrops(true);
    setDragEnabled(true);
    setDropIndicatorShown(true);
    setDefaultDropAction(Qt::CopyAction);
    setSelectionMode(QAbstractItemView::SingleSelection);
    setDragDropMode(QAbstractItemView::DragDrop);

    addSourcePath("/developer/kp/123.mp4");
    addSourcePath("/developer/kp/overlay.png");
    addSourcePath("/developer/kp/music.wav");
}

QStringList SourceAreaWidget::sourcePaths() const
{
    QStringList paths;
    paths.reserve(count());
    for (int i = 0; i < count(); ++i) {
        const auto *it = item(i);
        if (!it) {
            continue;
        }
        const QString path = it->data(Qt::UserRole).toString();
        if (!path.isEmpty()) {
            paths.push_back(path);
        }
    }
    return paths;
}

void SourceAreaWidget::setSourcePaths(const QStringList &paths)
{
    clear();
    for (const QString &path : paths) {
        addSourcePath(path.trimmed());
    }
}

void SourceAreaWidget::startDrag(Qt::DropActions supportedActions)
{
    auto *item = currentItem();
    if (!item) {
        return;
    }

    const QString sourcePath = item->data(Qt::UserRole).toString();
    if (sourcePath.isEmpty()) {
        return;
    }

    auto *mime = new QMimeData();
    mime->setText(sourcePath);
    mime->setUrls({QUrl::fromLocalFile(sourcePath)});

    auto *drag = new QDrag(this);
    drag->setMimeData(mime);
    drag->exec(supportedActions, Qt::CopyAction);
}

void SourceAreaWidget::dragEnterEvent(QDragEnterEvent *event)
{
    if (event->mimeData()->hasUrls() || event->mimeData()->hasText()) {
        event->acceptProposedAction();
        return;
    }
    QListWidget::dragEnterEvent(event);
}

void SourceAreaWidget::dragMoveEvent(QDragMoveEvent *event)
{
    if (event->mimeData()->hasUrls() || event->mimeData()->hasText()) {
        event->acceptProposedAction();
        return;
    }
    QListWidget::dragMoveEvent(event);
}

void SourceAreaWidget::dropEvent(QDropEvent *event)
{
    if (event->mimeData()->hasUrls()) {
        const auto urls = event->mimeData()->urls();
        for (const auto &url : urls) {
            if (url.isLocalFile()) {
                addSourcePath(url.toLocalFile());
            }
        }
        event->acceptProposedAction();
        return;
    }

    if (event->mimeData()->hasText()) {
        addSourcePath(event->mimeData()->text().trimmed());
        event->acceptProposedAction();
        return;
    }

    QListWidget::dropEvent(event);
}

void SourceAreaWidget::addSourcePath(const QString &path)
{
    if (path.isEmpty()) {
        return;
    }

    for (int i = 0; i < count(); ++i) {
        auto *it = item(i);
        if (it && it->data(Qt::UserRole).toString() == path) {
            return;
        }
    }

    const QFileInfo info(path);
    const QString label = info.fileName().isEmpty() ? path : info.fileName();
    auto *newItem = new QListWidgetItem(label, this);
    newItem->setToolTip(path);
    newItem->setData(Qt::UserRole, path);
    addItem(newItem);
}
