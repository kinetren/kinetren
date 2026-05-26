#include "bc.h"

#include <QPainter>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <deque>
#include <climits>
#include <limits>
#include <map>
#include <mutex>
#include <string>
#include <system_error>
#include <thread>
#include <utility>
#include <vector>

#include <QSize>

#include <avcpp/audioresampler.h>
#include <avcpp/avutils.h>
#include <avcpp/codec.h>
#include <avcpp/codeccontext.h>
#include <avcpp/dictionary.h>
#include <avcpp/formatcontext.h>
#include <avcpp/packet.h>
#include <avcpp/frame.h>
#include <avcpp/videorescaler.h>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/mathematics.h>
#include <libavutil/pixdesc.h>
}
#include "bc_internal.h"

namespace burstcon {

MonotonicPlaybackTimer::MonotonicPlaybackTimer(std::shared_ptr<IBurstcon> source)
    : source(std::move(source))
{
}

MonotonicPlaybackTimer::~MonotonicPlaybackTimer()
{
    stop();
}

void MonotonicPlaybackTimer::setSource(std::shared_ptr<IBurstcon> source)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->source = std::move(source);
}

void MonotonicPlaybackTimer::setMode(PlaybackMode mode, BurstIndex regionStart, BurstIndex regionEnd)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->mode = mode;
    this->regionStart = regionStart;
    this->regionEnd = regionEnd;
}

void MonotonicPlaybackTimer::setVideoRequestQuality(const BurstQuality &quality)
{
    std::lock_guard<std::mutex> lock(mutex);
    videoRequestQuality = quality;
}

void MonotonicPlaybackTimer::setPlaybackVideoFps(int fps)
{
    std::lock_guard<std::mutex> lock(mutex);
    playbackVideoFps = std::max(1, fps);
}

void MonotonicPlaybackTimer::setCallbacks(const Callbacks &callbacks)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->callbacks = callbacks;
}

void MonotonicPlaybackTimer::start(BurstIndex startIndex)
{
    stop();
    index.store(startIndex);
    running.store(true);
    timerThread = std::thread([this]() { runLoop(); });
}

void MonotonicPlaybackTimer::stop()
{
    running.store(false);
    if (timerThread.joinable()) {
        timerThread.join();
    }
}

bool MonotonicPlaybackTimer::isRunning() const
{
    return running.load();
}

BurstIndex MonotonicPlaybackTimer::currentIndex() const
{
    return index.load();
}

void MonotonicPlaybackTimer::runLoop()
{
    using Clock = std::chrono::steady_clock;
    auto loopStart = Clock::now();
    BurstIndex startIndex = index.load();
    BurstIndex lastVideoRequest = startIndex;
    const BurstIndex audioStep =
        std::max<BurstIndex>(1, (kBurstTimebaseHz * static_cast<BurstIndex>(kDefaultAudioSamplesPerBurst)) / 48000);
    BurstIndex nextAudioIndex = (startIndex / audioStep) * audioStep;
    auto nextAudioEmit = loopStart;
    const auto audioPeriod =
        std::chrono::microseconds(static_cast<int64_t>(audioStep) * 1000000LL / static_cast<int64_t>(kBurstTimebaseHz));
    {
        std::lock_guard<std::mutex> lock(mutex);
        const BurstIndex videoStep = outputVideoFrameStepTicks(playbackVideoFps);
        lastVideoRequest = startIndex - std::max<BurstIndex>(1, videoStep);
    }

    while (running.load()) {
        const auto now = Clock::now();
        const auto elapsedUs = std::chrono::duration_cast<std::chrono::microseconds>(now - loopStart).count();
        BurstIndex current = startIndex + static_cast<BurstIndex>((elapsedUs * kBurstTimebaseHz) / 1000000);

        PlaybackMode localMode;
        BurstIndex localRegionStart = 0;
        BurstIndex localRegionEnd = 0;
        std::shared_ptr<IBurstcon> localSource;
        Callbacks localCallbacks;
        BurstQuality localVideoRequestQuality;
        BurstIndex videoStep;
        {
            std::lock_guard<std::mutex> lock(mutex);
            localMode = mode;
            localRegionStart = regionStart;
            localRegionEnd = regionEnd;
            localSource = source;
            localCallbacks = callbacks;
            localVideoRequestQuality = videoRequestQuality;
            videoStep = outputVideoFrameStepTicks(playbackVideoFps);
        }
        if (localMode == PlaybackMode::Region || localMode == PlaybackMode::RegionLoop) {
            if (localRegionEnd > localRegionStart) {
                if (current >= localRegionEnd) {
                    if (localMode == PlaybackMode::RegionLoop) {
                        const BurstIndex span = localRegionEnd - localRegionStart;
                        current = localRegionStart + ((current - localRegionStart) % span);
                        loopStart = now;
                        startIndex = current;
                        lastVideoRequest = current - std::max<BurstIndex>(1, videoStep);
                        nextAudioIndex = (current / audioStep) * audioStep;
                        nextAudioEmit = now;
                    } else {
                        current = localRegionEnd;
                        running.store(false);
                    }
                } else if (current < localRegionStart) {
                    current = localRegionStart;
                }
            }
        }
        index.store(current);

        if (localSource) {
            // Feed audio on a steady clock with strictly sequential burst indices. The old path
            // jumped to (current/audioStep)*audioStep when the loop fell behind, skipping bursts
            // (clicks, "speed-up" glitches) and starving coverage stripes.
            const BurstIndex maxAudioIndex = (current / audioStep) * audioStep;
            if (nextAudioEmit + audioPeriod * 2 < now && nextAudioIndex < maxAudioIndex) {
                nextAudioIndex = maxAudioIndex;
                nextAudioEmit = now;
            }
            if (nextAudioEmit <= now && nextAudioIndex <= maxAudioIndex) {
                BurstRequest audioRequest;
                audioRequest.index = nextAudioIndex;
                AudioBurst ab;
                if (localSource->audioBurst(audioRequest, ab) && localCallbacks.onAudio) {
                    localCallbacks.onAudio(ab);
                }
                nextAudioIndex += audioStep;
                nextAudioEmit += audioPeriod;
            }

            BurstRequest videoRequest;
            BurstIndex nextVideoIndex = lastVideoRequest + videoStep;
            const BurstIndex maxVideoIndex = (current / videoStep) * videoStep;
            if (nextVideoIndex <= maxVideoIndex) {
                videoRequest.index = nextVideoIndex;
            } else {
                videoRequest.index = maxVideoIndex;
            }
            // Always use the project decode quality. Replacing 0×0 with sourceVideoQuality here
            // changes ConCache keys vs prefetch/warmup (which use decodeBurstQuality) and causes
            // constant cache misses, sparse video (~10–15 s picture updates), and audio starvation.
            videoRequest.quality = localVideoRequestQuality;
            if (videoRequest.index > lastVideoRequest && videoRequest.index <= maxVideoIndex) {
                VideoBurst vb;
                if (localSource->videoBurst(videoRequest, vb) && localCallbacks.onVideo) {
                    localCallbacks.onVideo(std::move(vb));
                }
                lastVideoRequest = videoRequest.index;
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(2));
    }
}

void MonotonicPlaybackTimer::preloadAhead(const std::shared_ptr<IBurstcon> &source,
                                          const BurstQuality &videoQuality,
                                          BurstIndex secondStart,
                                          BurstIndex videoStep,
                                          BurstIndex audioStep)
{
    if (!source) {
        return;
    }
    const BurstIndex end = secondStart + kBurstTimebaseHz;
    for (BurstIndex t = secondStart; t <= end; t += videoStep) {
        BurstRequest req;
        req.index = t;
        req.quality = videoQuality;
        VideoBurst out;
        (void)source->videoBurst(req, out);
    }
    for (BurstIndex t = secondStart; t <= end; t += audioStep) {
        BurstRequest req;
        req.index = t;
        AudioBurst out;
        (void)source->audioBurst(req, out);
    }
}

} // namespace burstcon
