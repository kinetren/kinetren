#include "playbackpipeline.h"

#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
#include <QAudioSink>
#include <QMediaDevices>
#else
#include <QAudioOutput>
#endif
#include <QAudioFormat>
#include <QIODevice>
#include <QPainter>
#include <QThreadPool>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <deque>
#include <limits>
#include <memory>
#include <thread>
#include <unordered_set>
#include <utility>

#include <avcpp/audioresampler.h>
#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/ffmpeg.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/videorescaler.h>

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr int kAudioBlockMs = 20;
constexpr int kAudioSamplesPerBlock = (kAudioSampleRate * kAudioBlockMs) / 1000;
constexpr size_t kAudioRingSlots = 12; // 12 * 20ms = 240ms
constexpr qint64 kAudioMaxAheadBlocks = static_cast<qint64>(kAudioRingSlots - 2);

constexpr int kVideoFps = 30;
constexpr int kVideoFrameMs = 1000 / kVideoFps;
constexpr size_t kVideoRingSlots = 7; // 7 * 33ms ~= 231ms
constexpr qint64 kVideoMaxAheadFrames = static_cast<qint64>(kVideoRingSlots - 2);
}

struct PlaybackPipeline::AudioRingBuffer {
    explicit AudioRingBuffer(size_t capacity)
        : keys(capacity, -1)
        , blocks(capacity, std::vector<int16_t>(kAudioSamplesPerBlock * kAudioChannels, 0))
    {
    }

    void setBlock(qint64 idx, const std::vector<int16_t> &samples)
    {
        const size_t slot = static_cast<size_t>(idx % static_cast<qint64>(blocks.size()));
        std::lock_guard<std::mutex> lock(mutex);
        keys[slot] = idx;
        blocks[slot] = samples;
    }

    bool getBlockExact(qint64 idx, std::vector<int16_t> &samples) const
    {
        const size_t slot = static_cast<size_t>(idx % static_cast<qint64>(blocks.size()));
        std::lock_guard<std::mutex> lock(mutex);
        if (keys[slot] != idx) {
            return false;
        }
        samples = blocks[slot];
        return !samples.empty();
    }

    mutable std::mutex mutex;
    std::vector<qint64> keys;
    std::vector<std::vector<int16_t>> blocks;
};

struct PlaybackPipeline::VideoRingBuffer {
    explicit VideoRingBuffer(size_t capacity)
        : slotFrameIndices(capacity, -1)
        , times(capacity, -1)
        , frames(capacity)
    {
    }

    void setReadCursorTime(qint64 timeMs)
    {
        const qint64 frameIndex = std::max<qint64>(0, timeMs / kVideoFrameMs);
        std::lock_guard<std::mutex> lock(mutex);
        readCursorFrameIndex = frameIndex;
    }

    void setFrame(qint64 timeMs, const QImage &img)
    {
        std::lock_guard<std::mutex> lock(mutex);
        const qint64 frameIndex = std::max<qint64>(0, timeMs / kVideoFrameMs);
        qint64 targetFrameIndex = std::max(frameIndex, readCursorFrameIndex);
        const qint64 maxBufferedFrameIndex =
            readCursorFrameIndex + static_cast<qint64>(frames.size()) - 1;
        if (targetFrameIndex > maxBufferedFrameIndex) {
            targetFrameIndex = maxBufferedFrameIndex;
        }
        const size_t slot =
            static_cast<size_t>(targetFrameIndex % static_cast<qint64>(frames.size()));
        slotFrameIndices[slot] = targetFrameIndex;
        times[slot] = timeMs;
        frames[slot] = img;
    }

    bool getFrameAtOrBefore(qint64 timeMs, qint64 maxBackMs, qint64 &foundTimeMs, QImage &img) const
    {
        const qint64 targetFrameIndex = std::max<qint64>(0, timeMs / kVideoFrameMs);
        const qint64 lower = std::max<qint64>(0, timeMs - maxBackMs);
        std::lock_guard<std::mutex> lock(mutex);
        qint64 bestFrameIndex = -1;
        qint64 bestTime = -1;
        int bestIndex = -1;
        for (size_t i = 0; i < frames.size(); ++i) {
            const qint64 slotFrameIndex = slotFrameIndices[i];
            if (slotFrameIndex < 0 || slotFrameIndex > targetFrameIndex) {
                continue;
            }
            const qint64 t = times[i];
            if (t < lower || t > timeMs) {
                continue;
            }
            if (frames[i].isNull()) {
                continue;
            }
            if (slotFrameIndex > bestFrameIndex || (slotFrameIndex == bestFrameIndex && t > bestTime)) {
                bestFrameIndex = slotFrameIndex;
                bestTime = t;
                bestIndex = static_cast<int>(i);
            }
        }
        if (bestIndex < 0) {
            return false;
        }
        foundTimeMs = bestTime;
        img = frames[static_cast<size_t>(bestIndex)];
        return true;
    }

    mutable std::mutex mutex;
    qint64 readCursorFrameIndex = 0;
    std::vector<qint64> slotFrameIndices;
    std::vector<qint64> times;
    std::vector<QImage> frames;
};

struct PlaybackPipeline::AudioWorker {
    std::string key;
    Clip clip;
    qint64 startOffsetMs = 0;
    int generation = 0;
    std::atomic<qint64> producedBlockIndex{-1};
    std::shared_ptr<std::atomic_bool> stopFlag;
    std::shared_ptr<AudioRingBuffer> ring;
};

struct PlaybackPipeline::VideoWorker {
    std::string key;
    Clip clip;
    qint64 startOffsetMs = 0;
    qint64 playbackStartMs = 0;
    std::atomic<int> frameDurationUs{33333};
    std::atomic<qint64> consumedTimeMs{0};
    std::atomic<qint64> producedTimeMs{-1};
    int generation = 0;
    std::shared_ptr<std::atomic_bool> stopFlag;
    std::shared_ptr<VideoRingBuffer> ring;
};

PlaybackPipeline::PlaybackPipeline(std::function<void(QImage &&)> onVideoFrameReady)
    : onVideoFrameReady(std::move(onVideoFrameReady))
{
}

PlaybackPipeline::~PlaybackPipeline()
{
    stop();
}

void PlaybackPipeline::start(qint64 startTimelineMs)
{
    timelineMs.store(startTimelineMs);
    ++generation;
    running.store(true);
    ensureAudioOutput();
    nextAudioBlock = startTimelineMs / kAudioBlockMs;
    nextVideoFrame = startTimelineMs / kVideoFrameMs;
    consumedAudioBlock.store(nextAudioBlock);
    consumedVideoFrame.store(nextVideoFrame);

    if (audioThread.joinable()) {
        audioThread.join();
    }
    if (videoThread.joinable()) {
        videoThread.join();
    }
    audioThread = std::thread([this]() {
        audioMixLoop();
    });
    videoThread = std::thread([this]() {
        videoComposeLoop();
    });
}

void PlaybackPipeline::stop()
{
    running.store(false);
    ++generation;
    if (audioThread.joinable()) {
        audioThread.join();
    }
    if (videoThread.joinable()) {
        videoThread.join();
    }
    {
        std::lock_guard<std::mutex> lock(audioMutex);
        for (auto &it : audioWorkers) {
            it.second->stopFlag->store(true);
        }
        audioWorkers.clear();
    }
    {
        std::lock_guard<std::mutex> lock(videoMutex);
        for (auto &it : videoWorkers) {
            it.second->stopFlag->store(true);
        }
        videoWorkers.clear();
    }
    shutdownAudioOutput();
}

void PlaybackPipeline::update(qint64 tMs, const std::vector<Clip> &videoClips, const std::vector<Clip> &audioClips)
{
    timelineMs.store(tMs);
    if (!running.load()) {
        return;
    }
    reconcileAudioWorkers(audioClips);
    reconcileVideoWorkers(videoClips);
}

std::string PlaybackPipeline::clipKey(const Clip &clip)
{
    return (clip.sourcePath + "::" + QString::number(clip.startMs)).toStdString();
}

void PlaybackPipeline::ensureAudioOutput()
{
    if (audioSink && audioDevice) {
        return;
    }
    QAudioFormat format;
    format.setSampleRate(kAudioSampleRate);
    format.setChannelCount(kAudioChannels);
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
    format.setSampleFormat(QAudioFormat::Int16);
    audioSink = new QAudioSink(QMediaDevices::defaultAudioOutput(), format);
#else
    format.setSampleSize(16);
    format.setCodec("audio/pcm");
    format.setByteOrder(QAudioFormat::LittleEndian);
    format.setSampleType(QAudioFormat::SignedInt);
    audioSink = new QAudioOutput(format);
#endif
    audioDevice = audioSink->start();
}

void PlaybackPipeline::shutdownAudioOutput()
{
    if (audioSink) {
        audioSink->stop();
        delete audioSink;
        audioSink = nullptr;
    }
    audioDevice = nullptr;
}

void PlaybackPipeline::reconcileAudioWorkers(const std::vector<Clip> &clips)
{
    const qint64 nowMs = timelineMs.load();
    std::unordered_set<std::string> active;
    for (const auto &clip : clips) {
        const auto key = clipKey(clip);
        active.insert(key);
        bool exists = false;
        {
            std::lock_guard<std::mutex> lock(audioMutex);
            exists = (audioWorkers.find(key) != audioWorkers.end());
        }
        if (!exists) {
            startAudioWorker(clip, nowMs);
        }
    }

    std::lock_guard<std::mutex> lock(audioMutex);
    for (auto it = audioWorkers.begin(); it != audioWorkers.end();) {
        if (active.find(it->first) != active.end()) {
            ++it;
            continue;
        }
        it->second->stopFlag->store(true);
        it = audioWorkers.erase(it);
    }
}

void PlaybackPipeline::reconcileVideoWorkers(const std::vector<Clip> &clips)
{
    const qint64 nowMs = timelineMs.load();
    std::unordered_set<std::string> active;
    for (const auto &clip : clips) {
        const auto key = clipKey(clip);
        active.insert(key);
        bool exists = false;
        {
            std::lock_guard<std::mutex> lock(videoMutex);
            exists = (videoWorkers.find(key) != videoWorkers.end());
        }
        if (!exists) {
            startVideoWorker(clip, nowMs);
        }
    }

    std::lock_guard<std::mutex> lock(videoMutex);
    for (auto it = videoWorkers.begin(); it != videoWorkers.end();) {
        if (active.find(it->first) != active.end()) {
            ++it;
            continue;
        }
        it->second->stopFlag->store(true);
        it = videoWorkers.erase(it);
    }
}

void PlaybackPipeline::startAudioWorker(const Clip &clip, qint64 timelineNowMs)
{
    auto worker = std::make_shared<AudioWorker>();
    worker->key = clipKey(clip);
    worker->clip = clip;
    worker->startOffsetMs = std::max<qint64>(0, timelineNowMs - clip.startMs);
    worker->generation = generation.load();
    worker->stopFlag = std::make_shared<std::atomic_bool>(false);
    worker->ring = std::make_shared<AudioRingBuffer>(kAudioRingSlots);

    const auto key = worker->key;
    {
        std::lock_guard<std::mutex> lock(audioMutex);
        audioWorkers[key] = worker;
    }

    QThreadPool::globalInstance()->start([this, worker]() {
        (void)decodeAudioWorker(*worker);
    });
}

void PlaybackPipeline::startVideoWorker(const Clip &clip, qint64 timelineNowMs)
{
    auto worker = std::make_shared<VideoWorker>();
    worker->key = clipKey(clip);
    worker->clip = clip;
    worker->startOffsetMs = std::max<qint64>(0, timelineNowMs - clip.startMs);
    worker->playbackStartMs = clip.startMs + worker->startOffsetMs;
    worker->consumedTimeMs.store(0);
    worker->generation = generation.load();
    worker->stopFlag = std::make_shared<std::atomic_bool>(false);
    worker->ring = std::make_shared<VideoRingBuffer>(kVideoRingSlots);

    const auto key = worker->key;
    {
        std::lock_guard<std::mutex> lock(videoMutex);
        videoWorkers[key] = worker;
    }

    QThreadPool::globalInstance()->start([this, worker]() {
        (void)decodeVideoWorker(*worker);
    });
}

void PlaybackPipeline::audioMixLoop()
{
    const int samplesPerBlock = kAudioSamplesPerBlock * kAudioChannels;
    const auto blockDuration = std::chrono::milliseconds(kAudioBlockMs);
    auto nextWake = std::chrono::steady_clock::now();

    while (running.load()) {
        if (!audioDevice) {
            std::this_thread::sleep_for(std::chrono::milliseconds(5));
            continue;
        }

        std::vector<std::shared_ptr<AudioWorker>> workers;
        {
            std::lock_guard<std::mutex> lock(audioMutex);
            workers.reserve(audioWorkers.size());
            for (auto &it : audioWorkers) {
                workers.push_back(it.second);
            }
        }

        std::vector<int32_t> mix(samplesPerBlock, 0);
        const qint64 blockMs = nextAudioBlock * kAudioBlockMs;
        for (const auto &worker : workers) {
            if (blockMs < worker->clip.startMs || blockMs >= (worker->clip.startMs + worker->clip.durationMs)) {
                continue;
            }
            for (int wait = 0; wait < 6 && running.load(); ++wait) {
                if (worker->producedBlockIndex.load() >= nextAudioBlock) {
                    break;
                }
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
            }
            std::vector<int16_t> block;
            const bool ready = worker->ring->getBlockExact(nextAudioBlock, block);
            if (!ready || static_cast<int>(block.size()) < samplesPerBlock) {
                continue;
            }
            for (int i = 0; i < samplesPerBlock; ++i) {
                mix[i] += block[i];
            }
        }

        std::vector<int16_t> out(samplesPerBlock, 0);
        for (int i = 0; i < samplesPerBlock; ++i) {
            out[i] = static_cast<int16_t>(std::clamp<int32_t>(
                mix[i],
                std::numeric_limits<int16_t>::min(),
                std::numeric_limits<int16_t>::max()));
        }

        const char *pcm = reinterpret_cast<const char *>(out.data());
        int bytes = static_cast<int>(out.size() * sizeof(int16_t));
        int offset = 0;
        while (offset < bytes && running.load()) {
            const qint64 written = audioDevice->write(pcm + offset, bytes - offset);
            if (written < 0) {
                break;
            }
            if (written == 0) {
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
                continue;
            }
            offset += static_cast<int>(written);
        }

        ++nextAudioBlock;
        consumedAudioBlock.store(nextAudioBlock);
        nextWake += blockDuration;
        std::this_thread::sleep_until(nextWake);
    }
}

void PlaybackPipeline::videoComposeLoop()
{
    auto nextWake = std::chrono::steady_clock::now();
    std::unordered_map<std::string, qint64> lastPresentedTimeByWorker;
    std::unordered_map<std::string, QImage> lastPresentedFrameByWorker;
    while (running.load()) {
        std::vector<std::shared_ptr<VideoWorker>> workers;
        {
            std::lock_guard<std::mutex> lock(videoMutex);
            workers.reserve(videoWorkers.size());
            for (auto &it : videoWorkers) {
                workers.push_back(it.second);
            }
        }

        const qint64 audioMasterMs = consumedAudioBlock.load() * kAudioBlockMs;
        QImage composed;
        bool hasAny = false;
        int composeFrameDurationUs = 33333;
        for (const auto &worker : workers) {
            if (audioMasterMs < worker->clip.startMs || audioMasterMs >= (worker->clip.startMs + worker->clip.durationMs)) {
                continue;
            }
            composeFrameDurationUs = std::min(composeFrameDurationUs, worker->frameDurationUs.load());
            const qint64 targetTimeMs = std::max<qint64>(0, audioMasterMs - worker->playbackStartMs);
            worker->consumedTimeMs.store(targetTimeMs);
            worker->ring->setReadCursorTime(targetTimeMs);
            for (int wait = 0; wait < 8 && running.load(); ++wait) {
                if (worker->producedTimeMs.load() >= targetTimeMs) {
                    break;
                }
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
            }
            QImage frame;
            qint64 foundTimeMs = -1;
            const bool ready = worker->ring->getFrameAtOrBefore(
                targetTimeMs,
                static_cast<qint64>(kVideoRingSlots * kVideoFrameMs),
                foundTimeMs,
                frame);
            const qint64 lastPresentedTime =
                lastPresentedTimeByWorker.count(worker->key) ? lastPresentedTimeByWorker[worker->key] : -1;

            if (!ready || frame.isNull()) {
                auto it = lastPresentedFrameByWorker.find(worker->key);
                if (it == lastPresentedFrameByWorker.end() || it->second.isNull()) {
                    continue;
                }
                frame = it->second;
            } else if (foundTimeMs < lastPresentedTime) {
                auto it = lastPresentedFrameByWorker.find(worker->key);
                if (it == lastPresentedFrameByWorker.end() || it->second.isNull()) {
                    continue;
                }
                frame = it->second;
            } else {
                lastPresentedTimeByWorker[worker->key] = foundTimeMs;
                lastPresentedFrameByWorker[worker->key] = frame;
            }

            if (!hasAny) {
                composed = frame.convertToFormat(QImage::Format_RGB888);
                hasAny = true;
                continue;
            }

            QPainter p(&composed);
            p.drawImage(composed.rect(), frame);
        }

        if (hasAny && onVideoFrameReady) {
            onVideoFrameReady(std::move(composed));
        }

        ++nextVideoFrame;
        consumedVideoFrame.store(nextVideoFrame);
        const int sleepUs = std::clamp(composeFrameDurationUs / 2, 4000, 16000);
        nextWake += std::chrono::microseconds(sleepUs);
        std::this_thread::sleep_until(nextWake);
    }
}

int PlaybackPipeline::decodeAudioWorker(AudioWorker &worker)
{
    using namespace av;

    std::error_code ec;
    FormatContext input;
    input.openInput(worker.clip.sourcePath.toStdString(), ec);
    if (ec) {
        return 1;
    }
    input.findStreamInfo(ec);
    if (ec) {
        return 1;
    }

    int audioStream = -1;
    Stream stream;
    for (size_t i = 0; i < input.streamsCount(); ++i) {
        auto st = input.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_AUDIO) {
            audioStream = static_cast<int>(i);
            stream = st;
            break;
        }
    }
    if (stream.isNull()) {
        return 1;
    }

    AudioDecoderContext dec(stream);
    Codec codec = findDecodingCodec(dec.raw()->codec_id);
    dec.setCodec(codec);
    dec.open(ec);
    if (ec) {
        return 1;
    }

    AudioResampler resampler(AV_CH_LAYOUT_STEREO,
                             kAudioSampleRate,
                             AV_SAMPLE_FMT_S16,
                             dec.channelLayout(),
                             dec.sampleRate(),
                             dec.sampleFormat(),
                             ec);
    if (ec) {
        return 1;
    }

    const int blockInterleaved = kAudioSamplesPerBlock * kAudioChannels;
    std::deque<int16_t> pending;
    qint64 nextBlock = (worker.clip.startMs + worker.startOffsetMs) / kAudioBlockMs;
    bool started = false;

    const auto popToRing = [&](bool getAll) -> int {
        while (true) {
            AudioSamples out(AV_SAMPLE_FMT_S16, 1024, AV_CH_LAYOUT_STEREO, kAudioSampleRate);
            const bool ok = resampler.pop(out, getAll, ec);
            if (ec) {
                return 1;
            }
            if (!ok || !out) {
                return 0;
            }
            const int count = out.samplesCount() * out.channelsCount();
            const int16_t *pcm = reinterpret_cast<const int16_t *>(out.data(0));
            for (int i = 0; i < count; ++i) {
                pending.push_back(pcm[i]);
            }
            while (static_cast<int>(pending.size()) >= blockInterleaved) {
                if (worker.stopFlag->load() || worker.generation != generation.load()) {
                    return 0;
                }
                while (worker.generation == generation.load() && !worker.stopFlag->load()) {
                    const qint64 timelineBlock = consumedAudioBlock.load();
                    const qint64 maxAhead = kAudioMaxAheadBlocks;
                    if (nextBlock <= timelineBlock + maxAhead) {
                        break;
                    }
                    std::this_thread::sleep_for(std::chrono::milliseconds(2));
                }
                std::vector<int16_t> block(blockInterleaved);
                for (int i = 0; i < blockInterleaved; ++i) {
                    block[i] = pending.front();
                    pending.pop_front();
                }
                worker.ring->setBlock(nextBlock, block);
                worker.producedBlockIndex.store(nextBlock);
                ++nextBlock;
            }
        }
    };

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        Packet pkt = input.readPacket(ec);
        if (!pkt) {
            // End of stream or temporary no packet; stop decode loop and flush decoder.
            break;
        }
        if (ec) {
            // Non-fatal packet read errors should not kill the track worker.
            ec.clear();
            continue;
        }
        if (pkt.streamIndex() != audioStream) {
            continue;
        }
        AudioSamples samples = dec.decode(pkt, ec);
        if (ec) {
            return 1;
        }
        if (!samples) {
            continue;
        }
        if (!samples.pts().isNoPts()) {
            const qint64 ptsMs = static_cast<qint64>(samples.pts().seconds() * 1000.0);
            if (!started && ptsMs < worker.startOffsetMs) {
                continue;
            }
        }
        started = true;
        resampler.push(samples, ec);
        if (ec) {
            return 1;
        }
        if (popToRing(false) != 0) {
            return 1;
        }
    }

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        AudioSamples samples = dec.decode(Packet(), ec);
        if (ec) {
            ec.clear();
            break;
        }
        if (!samples) {
            break;
        }
        resampler.push(samples, ec);
        if (ec || popToRing(false) != 0) {
            break;
        }
    }
    (void)popToRing(true);

    return 0;
}

int PlaybackPipeline::decodeVideoWorker(VideoWorker &worker)
{
    using namespace av;

    std::error_code ec;
    FormatContext input;
    input.openInput(worker.clip.sourcePath.toStdString(), ec);
    if (ec) {
        return 1;
    }
    input.findStreamInfo(ec);
    if (ec) {
        return 1;
    }

    int videoStream = -1;
    Stream stream;
    for (size_t i = 0; i < input.streamsCount(); ++i) {
        auto st = input.stream(i);
        if (st.mediaType() == AVMEDIA_TYPE_VIDEO) {
            videoStream = static_cast<int>(i);
            stream = st;
            break;
        }
    }
    if (stream.isNull()) {
        return 1;
    }

    VideoDecoderContext dec(stream);
    Codec codec = findDecodingCodec(dec.raw()->codec_id);
    dec.setCodec(codec);
    dec.setRefCountedFrames(true);
    Codec codecCtx{};
    dec.open({{"threads", "1"}}, codecCtx, ec);
    if (ec) {
        return 1;
    }

    const auto avgRate = stream.averageFrameRate();
    double fps = avgRate.getDouble();
    if (fps < 1.0) {
        fps = stream.frameRate().getDouble();
    }
    if (fps < 1.0) {
        fps = 30.0;
    }
    worker.frameDurationUs.store(static_cast<int>(std::llround(1000000.0 / fps)));

    VideoRescaler rescaler(dec.width(), dec.height(), AV_PIX_FMT_RGB24);
    qint64 lastPublishedTimeMs = -1;
    bool started = false;

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        Packet pkt = input.readPacket(ec);
        if (!pkt) {
            // End of stream or temporary no packet; stop decode loop and flush decoder.
            break;
        }
        if (ec) {
            ec.clear();
            continue;
        }
        if (pkt.streamIndex() != videoStream) {
            continue;
        }

        VideoFrame frame = dec.decode(pkt, ec);
        if (ec) {
            return 1;
        }
        if (frame.isNull() || !frame.isComplete()) {
            continue;
        }

        qint64 frameMs = 0;
        if (!frame.pts().isNoPts()) {
            frameMs = static_cast<qint64>(frame.pts().seconds() * 1000.0);
        }
        if (!started && frameMs < worker.startOffsetMs) {
            continue;
        }
        started = true;

        auto rgb = rescaler.rescale(frame);
        QImage img(rgb.width(), rgb.height(), QImage::Format_RGB888);
        const int dstStride = img.bytesPerLine();
        const int srcStride = rgb.raw()->linesize[0];
        const int copyStride = std::min(dstStride, srcStride);
        auto *src = rgb.data();
        auto *dst = img.bits();
        for (int y = 0; y < rgb.height(); ++y) {
            std::memcpy(dst + y * dstStride, src + y * srcStride, copyStride);
        }

        const qint64 relTimeMs = std::max<qint64>(0, frameMs - worker.startOffsetMs);
        if (relTimeMs < lastPublishedTimeMs) {
            // Never publish older timestamps (prevents backward flicker).
            continue;
        }
        while (worker.generation == generation.load() && !worker.stopFlag->load()) {
            const qint64 consumedTimeMs = worker.consumedTimeMs.load();
            const qint64 maxAheadMs = static_cast<qint64>(kVideoRingSlots * kVideoFrameMs - kVideoFrameMs);
            if (relTimeMs <= consumedTimeMs + maxAheadMs) {
                break;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
        worker.ring->setFrame(relTimeMs, img);
        worker.producedTimeMs.store(relTimeMs);
        lastPublishedTimeMs = relTimeMs;
    }

    while (!worker.stopFlag->load() && worker.generation == generation.load()) {
        VideoFrame frame = dec.decode(Packet(), ec);
        if (ec) {
            ec.clear();
            break;
        }
        if (!frame || frame.isNull() || !frame.isComplete()) {
            break;
        }

        qint64 frameMs = 0;
        if (!frame.pts().isNoPts()) {
            frameMs = static_cast<qint64>(frame.pts().seconds() * 1000.0);
        }
        if (frameMs < worker.startOffsetMs) {
            continue;
        }

        auto rgb = rescaler.rescale(frame);
        QImage img(rgb.width(), rgb.height(), QImage::Format_RGB888);
        const int dstStride = img.bytesPerLine();
        const int srcStride = rgb.raw()->linesize[0];
        const int copyStride = std::min(dstStride, srcStride);
        auto *src = rgb.data();
        auto *dst = img.bits();
        for (int y = 0; y < rgb.height(); ++y) {
            std::memcpy(dst + y * dstStride, src + y * srcStride, copyStride);
        }

        const qint64 relTimeMs = std::max<qint64>(0, frameMs - worker.startOffsetMs);
        if (relTimeMs < lastPublishedTimeMs) {
            continue;
        }

        while (worker.generation == generation.load() && !worker.stopFlag->load()) {
            const qint64 consumedTimeMs = worker.consumedTimeMs.load();
            const qint64 maxAheadMs = static_cast<qint64>(kVideoRingSlots * kVideoFrameMs - kVideoFrameMs);
            if (relTimeMs <= consumedTimeMs + maxAheadMs) {
                break;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
        worker.ring->setFrame(relTimeMs, img);
        worker.producedTimeMs.store(relTimeMs);
        lastPublishedTimeMs = relTimeMs;
    }

    return 0;
}
