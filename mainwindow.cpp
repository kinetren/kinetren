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
#include <QByteArray>
#include <QImage>
#include <QHBoxLayout>
#include <QLabel>
#include <QMetaObject>
#include <QIODevice>
#include <QPushButton>
#include <QThreadPool>
#include <QVBoxLayout>

#include <algorithm>
#include <chrono>
#include <cstring>
#include <deque>
#include <iostream>
#include <limits>
#include <thread>
#include <system_error>
#include <unordered_set>
#include <utility>

#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/ffmpeg.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/audioresampler.h>
#include <avcpp/videorescaler.h>

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr int kAudioBlockMs = 20;
constexpr int kAudioSamplesPerBlock = (kAudioSampleRate * kAudioBlockMs) / 1000;
constexpr size_t kAudioRingSlots = 256;

}

struct MainWindow::AudioRingBuffer {
    explicit AudioRingBuffer(size_t capacity)
        : blockKeys(capacity, -1)
        , blocks(capacity)
    {
    }

    void setBlock(qint64 blockIndex, const std::vector<int16_t> &samples)
    {
        const size_t slot = static_cast<size_t>(blockIndex % static_cast<qint64>(blockKeys.size()));
        std::lock_guard<std::mutex> lock(mutex);
        blockKeys[slot] = blockIndex;
        blocks[slot] = samples;
    }

    bool getBlock(qint64 blockIndex, std::vector<int16_t> &samples) const
    {
        const size_t slot = static_cast<size_t>(blockIndex % static_cast<qint64>(blockKeys.size()));
        std::lock_guard<std::mutex> lock(mutex);
        if (blockKeys[slot] != blockIndex) {
            return false;
        }
        samples = blocks[slot];
        return true;
    }

    mutable std::mutex mutex;
    std::vector<qint64> blockKeys;
    std::vector<std::vector<int16_t>> blocks;
};

struct MainWindow::AudioTrackWorker {
    QString sourcePath;
    qint64 clipStartMs = 0;
    qint64 clipDurationMs = 0;
    int generation = 0;
    std::shared_ptr<AudioRingBuffer> ring;
    std::shared_ptr<std::atomic_bool> stopFlag;
};

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
    if (activeAudio.empty()) {
        stopAudioPlayback();
        return;
    }

    ensureAudioPipelineStarted(timelineMs);
    updateAudioWorkers(timelineMs);
    mixAndOutputAudio(timelineMs);
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
    {
        std::lock_guard<std::mutex> lock(audioWorkersMutex);
        for (auto &entry : audioWorkers) {
            if (entry.second && entry.second->stopFlag) {
                entry.second->stopFlag->store(true);
            }
        }
        audioWorkers.clear();
    }
    currentAudioPath.clear();
    currentAudioClipStartMs = -1;
    shutdownAudioPipeline();
}

void MainWindow::ensureAudioPipelineStarted(qint64 timelineMs)
{
    if (audioSink && audioOutputDevice) {
        return;
    }

    QAudioFormat format;
    format.setSampleRate(kAudioSampleRate);
    format.setChannelCount(kAudioChannels);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    format.setSampleFormat(QAudioFormat::Int16);
    audioSink = new QAudioSink(QMediaDevices::defaultAudioOutput(), format, this);
#else
    format.setSampleSize(16);
    format.setCodec("audio/pcm");
    format.setByteOrder(QAudioFormat::LittleEndian);
    format.setSampleType(QAudioFormat::SignedInt);
    audioSink = new QAudioOutput(format, this);
#endif
    audioOutputDevice = audioSink->start();
    nextAudioBlockIndex = timelineMs / kAudioBlockMs;
}

void MainWindow::shutdownAudioPipeline()
{
    if (audioSink) {
        audioSink->stop();
        delete audioSink;
        audioSink = nullptr;
    }
    audioOutputDevice = nullptr;
    nextAudioBlockIndex = 0;
}

QString MainWindow::audioWorkerKey(const QString &path, qint64 clipStartMs) const
{
    return QString("%1::%2").arg(path).arg(clipStartMs);
}

void MainWindow::startAudioWorker(const QString &path, qint64 clipStartMs, qint64 clipDurationMs, qint64 timelineMs)
{
    const QString key = audioWorkerKey(path, clipStartMs);
    {
        std::lock_guard<std::mutex> lock(audioWorkersMutex);
        if (audioWorkers.find(key) != audioWorkers.end()) {
            return;
        }
    }

    auto worker = std::make_shared<AudioTrackWorker>();
    worker->sourcePath = path;
    worker->clipStartMs = clipStartMs;
    worker->clipDurationMs = clipDurationMs;
    worker->generation = audioPlaybackGeneration.load();
    worker->ring = std::make_shared<AudioRingBuffer>(kAudioRingSlots);
    worker->stopFlag = std::make_shared<std::atomic_bool>(false);

    {
        std::lock_guard<std::mutex> lock(audioWorkersMutex);
        audioWorkers[key] = worker;
    }

    const qint64 startOffsetMs = std::max<qint64>(0, timelineMs - clipStartMs);
    const qint64 baseBlockIndex = timelineMs / kAudioBlockMs;
    const std::string pathUtf8 = path.toStdString();
    auto ring = worker->ring;
    auto stopFlag = worker->stopFlag;
    const int generation = worker->generation;
    QThreadPool::globalInstance()->start([this, pathUtf8, startOffsetMs, baseBlockIndex, generation, ring, stopFlag]() {
        (void)decodeAudioToRing(pathUtf8, startOffsetMs, baseBlockIndex, generation, ring, stopFlag);
    });
}

void MainWindow::updateAudioWorkers(qint64 timelineMs)
{
    if (!timelineEditor) {
        return;
    }

    const auto active = timelineEditor->activeAudioAt(timelineMs);
    std::unordered_set<QString> activeKeys;
    for (const auto &clip : active) {
        activeKeys.insert(audioWorkerKey(clip.sourcePath, clip.startMs));
        startAudioWorker(clip.sourcePath, clip.startMs, clip.durationMs, timelineMs);
    }

    std::lock_guard<std::mutex> lock(audioWorkersMutex);
    for (auto it = audioWorkers.begin(); it != audioWorkers.end();) {
        if (activeKeys.find(it->first) != activeKeys.end()) {
            ++it;
            continue;
        }
        if (it->second && it->second->stopFlag) {
            it->second->stopFlag->store(true);
        }
        it = audioWorkers.erase(it);
    }
}

void MainWindow::mixAndOutputAudio(qint64 timelineMs)
{
    if (!timelineEditor || !audioOutputDevice) {
        return;
    }

    const qint64 targetBlockIndex = timelineMs / kAudioBlockMs;
    const int blockInterleavedSamples = kAudioSamplesPerBlock * kAudioChannels;
    while (nextAudioBlockIndex <= targetBlockIndex) {
        const qint64 blockTimeMs = nextAudioBlockIndex * kAudioBlockMs;
        const auto active = timelineEditor->activeAudioAt(blockTimeMs);

        std::vector<int32_t> mix(blockInterleavedSamples, 0);
        for (const auto &clip : active) {
            const QString key = audioWorkerKey(clip.sourcePath, clip.startMs);
            std::shared_ptr<AudioTrackWorker> worker;
            {
                std::lock_guard<std::mutex> lock(audioWorkersMutex);
                auto it = audioWorkers.find(key);
                if (it != audioWorkers.end()) {
                    worker = it->second;
                }
            }
            if (!worker || !worker->ring) {
                continue;
            }

            std::vector<int16_t> block;
            bool hasData = false;
            for (int retry = 0; retry < 6; ++retry) {
                hasData = worker->ring->getBlock(nextAudioBlockIndex, block);
                if (hasData) {
                    break;
                }
                // Ring-buffer retry loop: wait briefly for delayed worker data.
                std::this_thread::sleep_for(std::chrono::milliseconds(2));
            }
            if (!hasData || static_cast<int>(block.size()) < blockInterleavedSamples) {
                continue;
            }

            for (int i = 0; i < blockInterleavedSamples; ++i) {
                mix[i] += block[i];
            }
        }

        std::vector<int16_t> out(blockInterleavedSamples, 0);
        for (int i = 0; i < blockInterleavedSamples; ++i) {
            out[i] = static_cast<int16_t>(std::clamp<int32_t>(
                mix[i],
                std::numeric_limits<int16_t>::min(),
                std::numeric_limits<int16_t>::max()));
        }

        const char *pcm = reinterpret_cast<const char *>(out.data());
        int totalBytes = static_cast<int>(out.size() * sizeof(int16_t));
        int offset = 0;
        while (offset < totalBytes) {
            const qint64 written = audioOutputDevice->write(pcm + offset, totalBytes - offset);
            if (written < 0) {
                break;
            }
            if (written == 0) {
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
                continue;
            }
            offset += static_cast<int>(written);
        }

        ++nextAudioBlockIndex;
    }
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

int MainWindow::decodeAudioToRing(const std::string &uri,
                                  qint64 startOffsetMs,
                                  qint64 baseBlockIndex,
                                  int generation,
                                  const std::shared_ptr<AudioRingBuffer> &ring,
                                  const std::shared_ptr<std::atomic_bool> &stopFlag)
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

    AudioResampler resampler(AV_CH_LAYOUT_STEREO,
                             kAudioSampleRate,
                             AV_SAMPLE_FMT_S16,
                             decoder.channelLayout(),
                             decoder.sampleRate(),
                             decoder.sampleFormat(),
                             ec);
    if (ec) {
        cerr << "Can't init audio resampler\n";
        return 1;
    }

    const int interleavedSamplesPerBlock = kAudioSamplesPerBlock * kAudioChannels;
    qint64 nextBlockIndex = baseBlockIndex;
    std::deque<int16_t> pending;

    const auto pushResampledToRing = [&](bool getAll) -> int {
        while (true) {
            AudioSamples out(AV_SAMPLE_FMT_S16, 1024, AV_CH_LAYOUT_STEREO, kAudioSampleRate);
            const bool hasFrame = resampler.pop(out, getAll, ec);
            if (ec) {
                cerr << "Audio resampler pop error: " << ec << ", " << ec.message() << endl;
                return 1;
            }
            if (!hasFrame || !out) {
                return 0;
            }

            const int samplesToAppend = out.samplesCount() * out.channelsCount();
            const int16_t *pcm = reinterpret_cast<const int16_t *>(out.data(0));
            for (int i = 0; i < samplesToAppend; ++i) {
                pending.push_back(pcm[i]);
            }

            while (static_cast<int>(pending.size()) >= interleavedSamplesPerBlock) {
                if (generation != audioPlaybackGeneration.load() || (stopFlag && stopFlag->load())) {
                    return 0;
                }
                std::vector<int16_t> block(interleavedSamplesPerBlock);
                for (int i = 0; i < interleavedSamplesPerBlock; ++i) {
                    block[i] = pending.front();
                    pending.pop_front();
                }
                ring->setBlock(nextBlockIndex++, block);
            }
        }
    };

    while (true) {
        Packet pkt = inputContext.readPacket(ec);
        if (generation != audioPlaybackGeneration.load() || (stopFlag && stopFlag->load())) {
            return 0;
        }
        if (ec) {
            clog << "Audio packet reading error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (!pkt) {
            break;
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

        const int writeResult = pushResampledToRing(false);
        if (writeResult != 0) {
            return writeResult;
        }
    }

    while (true) {
        AudioSamples samples = decoder.decode(Packet(), ec);
        if (ec) {
            cerr << "Audio flush decode error: " << ec << ", " << ec.message() << endl;
            return 1;
        }
        if (!samples) {
            break;
        }
        resampler.push(samples, ec);
        if (ec) {
            cerr << "Audio resampler push error: " << ec << ", " << ec.message() << endl;
            return 1;
        }

        const int writeResult = pushResampledToRing(false);
        if (writeResult != 0) {
            return writeResult;
        }
    }

    const int writeResult = pushResampledToRing(true);
    if (writeResult != 0) {
        return writeResult;
    }

    if (!pending.empty()) {
        std::vector<int16_t> tail(interleavedSamplesPerBlock, 0);
        int i = 0;
        while (!pending.empty() && i < interleavedSamplesPerBlock) {
            tail[i++] = pending.front();
            pending.pop_front();
        }
        ring->setBlock(nextBlockIndex++, tail);
    }

    return 0;
}
