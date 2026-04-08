#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"

#include <QImage>
#include <QHBoxLayout>
#include <QLabel>
#include <QMetaObject>
#include <QPushButton>
#include <QVBoxLayout>

#include <cassert>
#include <cstring>
#include <iostream>
#include <system_error>
#include <utility>

#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/ffmpeg.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/videorescaler.h>

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(new Ui::MainWindow)
{
    ui->setupUi(this);
    setupTimelineEditorUi();
}

MainWindow::~MainWindow()
{
    delete ui;
}

void MainWindow::startPlayback()
{
    static const std::string kVideoPath = "/developer/kp/123.mp4";
    (void)decodeVideoFile(kVideoPath);
}

void MainWindow::setupTimelineEditorUi()
{
    auto *sourceLayout = new QVBoxLayout(ui->widget_3);
    sourceLayout->setContentsMargins(6, 6, 6, 6);
    sourceLayout->setSpacing(6);

    auto *sourceTitle = new QLabel("Source area (drop files here, then drag to timeline)", ui->widget_3);
    sourceLayout->addWidget(sourceTitle);

    sourceArea = new SourceAreaWidget(ui->widget_3);
    sourceLayout->addWidget(sourceArea, 1);

    auto *timelineLayout = new QVBoxLayout(ui->widget_2);
    timelineLayout->setContentsMargins(6, 6, 6, 6);
    timelineLayout->setSpacing(6);

    auto *controlsLayout = new QHBoxLayout();
    timelinePlayPauseButton = new QPushButton("Pause", ui->widget_2);
    auto *markerButton = new QPushButton("Add marker", ui->widget_2);
    timelineOverlayLabel = new QLabel("Overlay @ 0.00s | Video: - | Audio layers: 0", ui->widget_2);
    controlsLayout->addWidget(timelinePlayPauseButton);
    controlsLayout->addWidget(markerButton);
    controlsLayout->addWidget(timelineOverlayLabel, 1);
    timelineLayout->addLayout(controlsLayout);

    timelineEditor = new TimelineEditor(ui->widget_2);
    timelineLayout->addWidget(timelineEditor, 1);

    timelineTimer.setInterval(40);
    connect(&timelineTimer, &QTimer::timeout, this, &MainWindow::onTimelineTick);
    connect(timelinePlayPauseButton, &QPushButton::clicked, this, &MainWindow::toggleTimelinePlayback);
    connect(markerButton, &QPushButton::clicked, timelineEditor, &TimelineEditor::addMarkerAtCursor);
    connect(timelineEditor, &TimelineEditor::overlayInfoChanged, timelineOverlayLabel, &QLabel::setText);
}

void MainWindow::toggleTimelinePlayback()
{
    if (!timelineEditor) {
        return;
    }

    if (!timelinePlaying) {
        timelinePlaying = true;
        timelineReturnCursorMs = timelineEditor->cursorMs();
        timelinePlayPauseButton->setText("Pause (back)");
        timelineTimer.start();
        return;
    }

    timelinePlaying = false;
    timelineTimer.stop();
    timelineEditor->setCursorMs(timelineReturnCursorMs);
    timelinePlayPauseButton->setText("Pause");
}

void MainWindow::onTimelineTick()
{
    if (!timelineEditor || !timelinePlaying) {
        return;
    }
    timelineEditor->setCursorMs(timelineEditor->cursorMs() + timelineTimer.interval());
}

void MainWindow::paintFrame(av::VideoFrame &&frame)
{
    QImage image(frame.width(), frame.height(), QImage::Format_RGB888);
    uchar *buffer = image.bits();
    const int lineSize = image.bytesPerLine();

    auto *data = frame.data();
    const int srcBitsPerPixel = frame.pixelFormat().bitsPerPixel();
    const int dstBitsPerPixel = (lineSize / frame.width()) * 8;
    assert(srcBitsPerPixel == dstBitsPerPixel);

    for (int i = 0; i < frame.height(); ++i) {
        auto *srcLine = data + (i * lineSize);
        auto *dstLine = buffer + (i * lineSize);
        std::memcpy(dstLine, srcLine, lineSize);
    }

    auto *frameView = ui->frame;
    QMetaObject::invokeMethod(
        frameView,
        [frameView, image = std::move(image)]() mutable {
            frameView->paintIt(std::move(image));
        },
        Qt::QueuedConnection);
}

int MainWindow::decodeVideoFile(const std::string &uri)
{
    using namespace av;
    using std::cerr;
    using std::clog;
    using std::endl;

    av::init();
    av::setFFmpegLoggingLevel(AV_LOG_DEBUG);

    int videoStream = -1;
    VideoDecoderContext decoder;
    Stream videoStreamInfo;
    std::error_code ec;

    FormatContext inputContext;
    inputContext.openInput(uri, ec);
    if (ec) {
        cerr << "Can't open input\n";
        return 1;
    }

    cerr << "Streams: " << inputContext.streamsCount() << endl;
    inputContext.findStreamInfo(ec);
    if (ec) {
        cerr << "Can't find streams: " << ec << ", " << ec.message() << endl;
        return 1;
    }

    for (size_t i = 0; i < inputContext.streamsCount(); ++i) {
        auto st = inputContext.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_VIDEO) {
            videoStream = i;
            videoStreamInfo = st;
            break;
        }
    }

    if (videoStreamInfo.isNull()) {
        cerr << "Video stream not found\n";
        return 1;
    }

    decoder = VideoDecoderContext(videoStreamInfo);
    Codec codec = findDecodingCodec(decoder.raw()->codec_id);
    decoder.setCodec(codec);
    decoder.setRefCountedFrames(true);
    Codec codecContext{};
    decoder.open({{"threads", "1"}}, codecContext, ec);
    if (ec) {
        cerr << "Can't open codec\n";
        return 1;
    }

    VideoRescaler rescaler(decoder.width(), decoder.height(), AV_PIX_FMT_RGB24);
    while (Packet pkt = inputContext.readPacket(ec)) {
        if (ec) {
            clog << "Packet reading error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (pkt.streamIndex() != videoStream) {
            continue;
        }

        VideoFrame frame = decoder.decode(pkt, ec);
        if (ec) {
            cerr << "Error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (frame.isNull() || !frame.isComplete()) {
            continue;
        }

        auto frameRescaled = rescaler.rescale(frame);
        paintFrame(std::move(frameRescaled));
    }

    while (true) {
        VideoFrame frame = decoder.decode(Packet(), ec);
        if (ec) {
            cerr << "Error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (!frame) {
            break;
        }
    }

    return 0;
}
