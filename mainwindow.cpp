#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "sourceareawidget.h"
#include "timelineeditor.h"

#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
#include <QAudioSink>
#include <QMediaDevices>
#else
#include <QAudioOutput>
#endif
#include <QAudioFormat>
#include <QImage>
#include <QHBoxLayout>
#include <QLabel>
#include <QMetaObject>
#include <QPushButton>
#include <QThreadPool>
#include <QVBoxLayout>

#include <algorithm>
#include <chrono>
#include <cstring>
#include <iostream>
#include <thread>
#include <system_error>
#include <utility>

#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/ffmpeg.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/audioresampler.h>
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
    stopMediaPlayback();
    delete ui;
}

void MainWindow::startPlayback()
{
    syncTimelinePlayback();
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
        syncTimelinePlayback();
        timelineTimer.start();
        return;
    }

    timelinePlaying = false;
    timelineTimer.stop();
    stopMediaPlayback();
    timelineEditor->setCursorMs(timelineReturnCursorMs);
    timelinePlayPauseButton->setText("Pause");
}

void MainWindow::onTimelineTick()
{
    if (!timelineEditor || !timelinePlaying) {
        return;
    }
    timelineEditor->setCursorMs(timelineEditor->cursorMs() + timelineTimer.interval());
    syncTimelinePlayback();
}

void MainWindow::syncTimelinePlayback()
{
    if (!timelineEditor) {
        return;
    }

    const qint64 timelineMs = timelineEditor->cursorMs();
    const auto activeVideo = timelineEditor->activeVideoAt(timelineMs);
    if (activeVideo.has_value()) {
        startVideoPlayback(activeVideo->sourcePath, activeVideo->startMs, timelineMs);
    } else {
        currentVideoPath.clear();
        currentVideoClipStartMs = -1;
        ++videoPlaybackGeneration;
    }

    const auto activeAudio = timelineEditor->activeAudioAt(timelineMs);
    if (!activeAudio.empty()) {
        const auto &clip = activeAudio.front();
        startAudioPlayback(clip.sourcePath, clip.startMs, timelineMs);
    } else {
        stopAudioPlayback();
    }
}

void MainWindow::stopMediaPlayback()
{
    ++videoPlaybackGeneration;
    currentVideoPath.clear();
    currentVideoClipStartMs = -1;
    stopAudioPlayback();
}

void MainWindow::stopAudioPlayback()
{
    ++audioPlaybackGeneration;
    currentAudioPath.clear();
    currentAudioClipStartMs = -1;
}

void MainWindow::startVideoPlayback(const QString &path, qint64 clipStartMs, qint64 timelineMs)
{
    if (path.isEmpty()) {
        ++videoPlaybackGeneration;
        currentVideoPath.clear();
        currentVideoClipStartMs = -1;
        return;
    }

    if (path == currentVideoPath && clipStartMs == currentVideoClipStartMs) {
        return;
    }

    const int generation = ++videoPlaybackGeneration;
    currentVideoPath = path;
    currentVideoClipStartMs = clipStartMs;
    const qint64 startOffsetMs = std::max<qint64>(0, timelineMs - clipStartMs);
    const std::string pathUtf8 = path.toStdString();

    QThreadPool::globalInstance()->start([this, pathUtf8, startOffsetMs, generation]() {
        (void)decodeVideoFile(pathUtf8, startOffsetMs, generation);
    });
}

void MainWindow::startAudioPlayback(const QString &path, qint64 clipStartMs, qint64 timelineMs)
{
    if (path.isEmpty()) {
        stopAudioPlayback();
        return;
    }

    if (path == currentAudioPath && clipStartMs == currentAudioClipStartMs) {
        return;
    }

    const int generation = ++audioPlaybackGeneration;
    currentAudioPath = path;
    currentAudioClipStartMs = clipStartMs;
    const qint64 startOffsetMs = std::max<qint64>(0, timelineMs - clipStartMs);
    const std::string pathUtf8 = path.toStdString();
    QThreadPool::globalInstance()->start([this, pathUtf8, startOffsetMs, generation]() {
        (void)decodeAudioFile(pathUtf8, startOffsetMs, generation);
    });
}

void MainWindow::paintFrame(av::VideoFrame &&frame)
{
    QImage image(frame.width(), frame.height(), QImage::Format_RGB888);
    uchar *buffer = image.bits();
    const int dstLineSize = image.bytesPerLine();
    const int srcLineSize = frame.raw()->linesize[0];
    const int copyLineSize = std::min(dstLineSize, srcLineSize);

    auto *data = frame.data();

    for (int i = 0; i < frame.height(); ++i) {
        auto *srcLine = data + (i * srcLineSize);
        auto *dstLine = buffer + (i * dstLineSize);
        std::memset(dstLine, 0, dstLineSize);
        std::memcpy(dstLine, srcLine, copyLineSize);
    }

    auto *frameView = ui->frame;
    QMetaObject::invokeMethod(
        frameView,
        [frameView, image = std::move(image)]() mutable {
            frameView->paintIt(std::move(image));
        },
        Qt::QueuedConnection);
}

int MainWindow::decodeVideoFile(const std::string &uri, qint64 startOffsetMs, int generation)
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
    const auto playbackStart = std::chrono::steady_clock::now();
    bool firstPtsSet = false;
    double firstPtsSeconds = 0.0;

    while (Packet pkt = inputContext.readPacket(ec)) {
        if (generation != videoPlaybackGeneration.load()) {
            return 0;
        }
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

        const auto framePts = frame.pts();
        if (!framePts.isNoPts()) {
            const qint64 frameMs = static_cast<qint64>(framePts.seconds() * 1000.0);
            if (frameMs < startOffsetMs) {
                continue;
            }
        }

        if (!framePts.isNoPts()) {
            const double framePtsSeconds = framePts.seconds();
            if (!firstPtsSet) {
                firstPtsSet = true;
                firstPtsSeconds = framePtsSeconds;
            }

            const auto targetMs = static_cast<qint64>((framePtsSeconds - firstPtsSeconds) * 1000.0);
            const auto elapsedMs = std::chrono::duration_cast<std::chrono::milliseconds>(
                                       std::chrono::steady_clock::now() - playbackStart)
                                       .count();
            if (targetMs > elapsedMs) {
                std::this_thread::sleep_for(std::chrono::milliseconds(targetMs - elapsedMs));
            }
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

int MainWindow::decodeAudioFile(const std::string &uri, qint64 startOffsetMs, int generation)
{
    using namespace av;
    using std::cerr;
    using std::clog;
    using std::endl;

    av::init();

    int audioStream = -1;
    AudioDecoderContext decoder;
    Stream audioStreamInfo;
    std::error_code ec;

    FormatContext inputContext;
    inputContext.openInput(uri, ec);
    if (ec) {
        cerr << "Can't open audio input\n";
        return 1;
    }

    inputContext.findStreamInfo(ec);
    if (ec) {
        cerr << "Can't find streams: " << ec << ", " << ec.message() << endl;
        return 1;
    }

    for (size_t i = 0; i < inputContext.streamsCount(); ++i) {
        auto st = inputContext.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_AUDIO) {
            audioStream = static_cast<int>(i);
            audioStreamInfo = st;
            break;
        }
    }

    if (audioStreamInfo.isNull()) {
        cerr << "Audio stream not found\n";
        return 1;
    }

    decoder = AudioDecoderContext(audioStreamInfo);
    Codec codec = findDecodingCodec(decoder.raw()->codec_id);
    decoder.setCodec(codec);
    decoder.open(ec);
    if (ec) {
        cerr << "Can't open audio codec\n";
        return 1;
    }

    const int outSampleRate = 48000;
    const uint64_t outChannelLayout = AV_CH_LAYOUT_STEREO;
    const SampleFormat outSampleFormat = AV_SAMPLE_FMT_S16;

    AudioResampler resampler(outChannelLayout,
                             outSampleRate,
                             outSampleFormat,
                             decoder.channelLayout(),
                             decoder.sampleRate(),
                             decoder.sampleFormat(),
                             ec);
    if (ec) {
        cerr << "Can't init audio resampler\n";
        return 1;
    }

    QAudioFormat audioFormat;
    audioFormat.setSampleRate(outSampleRate);
    audioFormat.setChannelCount(2);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    audioFormat.setSampleFormat(QAudioFormat::Int16);
    QAudioSink sink(QMediaDevices::defaultAudioOutput(), audioFormat);
#else
    audioFormat.setSampleSize(16);
    audioFormat.setCodec("audio/pcm");
    audioFormat.setByteOrder(QAudioFormat::LittleEndian);
    audioFormat.setSampleType(QAudioFormat::SignedInt);
    QAudioOutput sink(audioFormat);
#endif
    QIODevice *audioDevice = sink.start();
    if (!audioDevice) {
        cerr << "Can't start audio output\n";
        return 1;
    }

    while (Packet pkt = inputContext.readPacket(ec)) {
        if (generation != audioPlaybackGeneration.load()) {
            return 0;
        }
        if (ec) {
            clog << "Audio packet reading error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (pkt.streamIndex() != audioStream) {
            continue;
        }

        AudioSamples samples = decoder.decode(pkt, ec);
        if (ec) {
            cerr << "Audio decode error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (!samples) {
            continue;
        }

        const auto samplePts = samples.pts();
        if (!samplePts.isNoPts()) {
            const qint64 sampleMs = static_cast<qint64>(samplePts.seconds() * 1000.0);
            if (sampleMs < startOffsetMs) {
                continue;
            }
        }

        resampler.push(samples, ec);
        if (ec) {
            cerr << "Audio resampler push error: " << ec << ", " << ec.message() << endl;
            return 1;
        }

        while (true) {
            AudioSamples out(outSampleFormat, 1024, outChannelLayout, outSampleRate);
            const bool hasFrame = resampler.pop(out, false, ec);
            if (ec) {
                cerr << "Audio resampler pop error: " << ec << ", " << ec.message() << endl;
                return 1;
            }
            if (!hasFrame || !out) {
                break;
            }

            const int bytesToWrite = out.samplesCount() * out.channelsCount() * 2;
            const char *pcm = reinterpret_cast<const char *>(out.data(0));
            int offset = 0;
            while (offset < bytesToWrite) {
                if (generation != audioPlaybackGeneration.load()) {
                    return 0;
                }
                const qint64 written = audioDevice->write(pcm + offset, bytesToWrite - offset);
                if (written <= 0) {
                    std::this_thread::sleep_for(std::chrono::milliseconds(5));
                    continue;
                }
                offset += static_cast<int>(written);
            }
        }
    }

    return 0;
}
