#ifndef PREVIEWER_H
#define PREVIEWER_H

#include <QObject>
#include <QWidget>
#include <QPainter>
#include <qdebug.h>
#include <chrono>
#include <thread>
#include <QTimerEvent>


const int rapibuf_cnt = 20;
typedef struct
{
    std::atomic_int has;
    QImage img;
} RPFrame;
class Previewer : public QWidget
{
    Q_OBJECT
    RPFrame rapibu[rapibuf_cnt] = {0};
    int rapibu_i = 0;
    int rapibu_p_next = 0;
    int rapibu_p_prev = -1;
    int paint_timer;
public:
    explicit Previewer(QWidget *parent = nullptr)
        : QWidget{parent}
    {
        using namespace std::chrono_literals;
        paint_timer = startTimer(24ms, Qt::VeryCoarseTimer);
        if (paint_timer < 0)
            abort();
    }
    void timerEvent(QTimerEvent *event) override
    {
        /*
        if (event->timerId() != paint_timer)
            return;*/
        for (int i = 0; i < rapibuf_cnt; i++)
        {
            if (rapibu[i].has == 2)
            {
                update();
                return;
            }
        }
    }

    void paintIt(QImage&& img)
    {
        auto i = (rapibu_i++)% rapibuf_cnt;
        while (rapibu[i].has != 0)
        {
            std::this_thread::sleep_for(std::chrono::milliseconds(24));
        }
        rapibu[i].has = 1;
        rapibu[i].img = std::move(img);
        rapibu[i].has = 2;
        //update();
    }

    std::atomic<uint64_t> last_paint{0};
    void paintEvent(QPaintEvent *event) override {

        int i = (rapibu_p_next) % rapibuf_cnt;

        bool need_old = false;
        auto currtime = std::chrono::high_resolution_clock::now().time_since_epoch().count();
        if (last_paint > 0) {
            auto diff = currtime - last_paint;
            if (diff < 25'000'000)
                need_old = true;

        }

        if (need_old)
        {
            if (rapibu_p_prev >= 0)
                i = rapibu_p_prev;
            else
                rapibu_p_prev = rapibu_p_next = i = 0;
        }
        else
        {
            if (rapibu_p_prev >= 0 && rapibu_p_prev != i)
                rapibu[rapibu_p_prev].has = 0;
            last_paint = currtime;
        }

        if (rapibu[i].has != 2 && rapibu[i].has != 4)
            return;
        rapibu[i].has = 3;
        {
            QPainter painter(this);
            if (painter.isActive()) {
                // painter активен, можно рисовать
                QPixmap pixmap = QPixmap::fromImage(rapibu[i].img, Qt::ColorOnly);
                //painter.drawPixmap(0, 0, pixmap);
                painter.drawPixmap(0, 0, std::min(pixmap.width(), width()), std::min(pixmap.height(), height()), pixmap);
            } else {
                qDebug() << "QPainter не активен";
            }
            painter.end();
        }
        rapibu[i].has = 4;
        rapibu_p_prev = i;
        rapibu_p_next = i + 1;
    }

signals:
};

#endif // PREVIEWER_H
