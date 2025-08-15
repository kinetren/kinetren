#include "mainwindow.h"

#include <QApplication>
#include <QThreadPool>

int main(int argc, char *argv[])
{
    QApplication a(argc, argv);
    MainWindow w;
    w.show();
    QThreadPool::globalInstance()->start(std::bind(&MainWindow::test, &w));
    return a.exec();
}
