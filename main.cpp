#include "mainwindow.h"

#include <QApplication>
#include <QThreadPool>

int main(int argc, char *argv[])
{
    QApplication a(argc, argv);
    MainWindow w;
    w.show();
    QThreadPool::globalInstance()->start([&w]() { w.startPlayback(); });
    return a.exec();
}
