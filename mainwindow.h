#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QTimer>
#include <avcpp/av.h>
#include <string>

QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE

namespace av {
class VideoFrame;
}

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow() override;

    void startPlayback();

private:
    void setupTimelineEditorUi();
    void toggleTimelinePlayback();
    void onTimelineTick();

    int decodeVideoFile(const std::string &uri);
    void paintFrame(av::VideoFrame &&frame);

    class SourceAreaWidget *sourceArea = nullptr;
    class TimelineEditor *timelineEditor = nullptr;
    class QPushButton *timelinePlayPauseButton = nullptr;
    class QLabel *timelineOverlayLabel = nullptr;
    QTimer timelineTimer;
    bool timelinePlaying = false;
    qint64 timelineReturnCursorMs = 0;

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
