#ifndef SOURCEAREAWIDGET_H
#define SOURCEAREAWIDGET_H

#include <QListWidget>

class SourceAreaWidget : public QListWidget
{
    Q_OBJECT

public:
    explicit SourceAreaWidget(QWidget *parent = nullptr);

protected:
    void startDrag(Qt::DropActions supportedActions) override;
    void dragEnterEvent(QDragEnterEvent *event) override;
    void dragMoveEvent(QDragMoveEvent *event) override;
    void dropEvent(QDropEvent *event) override;

private:
    void addSourcePath(const QString &path);
};

#endif // SOURCEAREAWIDGET_H
