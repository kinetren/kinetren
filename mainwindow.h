#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <avcpp/av.h>
#include <string>

QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow() override;

    void startPlayback();

private:
    int decodeVideoFile(const std::string &uri);
    void paintFrame(av::VideoFrame &&frame);

    Ui::MainWindow *ui;
};

#endif // MAINWINDOW_H
