#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>

#include <avcpp/av.h>
#include <avcpp/ffmpeg.h>
#include <avcpp/codec.h>
#include <avcpp/packet.h>
#include <avcpp/videorescaler.h>
#include <avcpp/audioresampler.h>
#include <avcpp/avutils.h>

// API2
#include <avcpp/formatcontext.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>

#include <QImage>
#include <QPixmap>
#include <QPainter>
#include <QDebug>


QT_BEGIN_NAMESPACE
namespace Ui {
class MainWindow;
}
QT_END_NAMESPACE
#include "ui_mainwindow.h"

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    MainWindow(QWidget *parent = nullptr);
    ~MainWindow();
    void paint(av::VideoFrame& frame)
    {

        // ... настройте avcpp и декодируйте кадр

        // Преобразование в QImage
        QImage image(frame.width(), frame.height(), QImage::Format_RGB888);//Format_RGB888
        uchar* buffer = image.bits();
        int linesize = image.bytesPerLine();


        // Для упрощения, предположим, что frame->format == AV_PIX_FMT_BGR24
        // и что stride равен linesize
        uint8_t *data = frame.data();
        for (int i = 0; i < frame.height(); ++i) {

            uchar *src_line = data + (i * frame.width());
            uchar *dst_line = buffer + i * linesize;
            memcpy(dst_line, src_line, linesize);//frame.width() * 3);
        }

       // frame.copyToBuffer(buffer, image.bytesPerLine()*frame.height(), 0);
        this->ui->frame->paintIt(std::move(image));
        /*
        // Вывод изображения
        QPixmap pixmap = QPixmap::fromImage(image);
        QPainter painter(this->ui->frame);
        painter.drawPixmap(0, 0, pixmap);
        painter.end();*/

    }
    void test()
    {
        test0();
    }
int test0()
    {
    sleep(1);
        using namespace std;
        using namespace av;
        av::init();
        av::setFFmpegLoggingLevel(AV_LOG_DEBUG);

        string uri {"/developer/kp/a.mp4"};

        ssize_t      videoStream = -1;
        VideoDecoderContext vdec;
        Stream      vst;
        error_code   ec;

        int count = 0;

        {

            FormatContext ictx;

            ictx.openInput(uri, ec);
            if (ec) {
                cerr << "Can't open input\n";
                return 1;
            }

            cerr << "Streams: " << ictx.streamsCount() << endl;

            ictx.findStreamInfo(ec);
            if (ec) {
                cerr << "Can't find streams: " << ec << ", " << ec.message() << endl;
                return 1;
            }

            for (size_t i = 0; i < ictx.streamsCount(); ++i) {
                auto st = ictx.stream(i);
                if (st.mediaType() == AVMEDIA_TYPE_VIDEO) {
                    videoStream = i;
                    vst = st;
                    break;
                }
            }

            cerr << videoStream << endl;

            if (vst.isNull()) {
                cerr << "Video stream not found\n";
                return 1;
            }

            if (vst.isValid()) {
                vdec = VideoDecoderContext(vst);


                Codec codec = findDecodingCodec(vdec.raw()->codec_id);

                vdec.setCodec(codec);
                vdec.setRefCountedFrames(true);

                vdec.open({{"threads", "1"}}, Codec(), ec);
                //vdec.open(ec);
                if (ec) {
                    cerr << "Can't open codec\n";
                    return 1;
                }
            }


            while (Packet pkt = ictx.readPacket(ec)) {
                if (ec) {
                    clog << "Packet reading error: " << ec << ", " << ec.message() << endl;
                    return 1;
                }

                if (pkt.streamIndex() != videoStream) {
                    continue;
                }

                auto ts = pkt.ts();
                clog << "Read packet: " << ts << " / " << ts.seconds() << " / " << pkt.timeBase() << " / st: " << pkt.streamIndex() << endl;

                VideoFrame frame = vdec.decode(pkt, ec);

                count++;
                //if (count > 100)
                //    break;

                if (ec) {
                    cerr << "Error: " << ec << ", " << ec.message() << endl;
                    return 1;
                } else if (!frame) {
                    cerr << "Empty frame\n";
                    //continue;
                }

                ts = frame.pts();
                paint(frame);
                clog << "  Frame: " << frame.width() << "x" << frame.height() << ", size=" << frame.size() << ", ts=" << ts << ", tm: " << ts.seconds() << ", tb: " << frame.timeBase() << ", ref=" << frame.isReferenced() << ":" << frame.refCount() << endl;

            }

            clog << "Flush frames;\n";
            while (true) {
                VideoFrame frame = vdec.decode(Packet(), ec);
                if (ec) {
                    cerr << "Error: " << ec << ", " << ec.message() << endl;
                    return 1;
                }
                if (!frame)
                    break;
                auto ts = frame.pts();
                clog << "  Frame: " << frame.width() << "x" << frame.height() << ", size=" << frame.size() << ", ts=" << ts << ", tm: " << ts.seconds() << ", tb: " << frame.timeBase() << ", ref=" << frame.isReferenced() << ":" << frame.refCount() << endl;
            }

            // NOTE: stream decodec must be closed/destroyed before
            //ictx.close();
            //vdec.close();
        }
        return 0;
    }
private:
    Ui::MainWindow *ui;
};
#endif // MAINWINDOW_H
