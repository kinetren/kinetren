#include "burstcon.h"

#include <QCoreApplication>
#include <QImage>

#include <cassert>
#include <chrono>
#include <memory>
#include <algorithm>
#include <thread>

using namespace burstcon;

namespace {

class FakeBurstcon : public IBurstcon
{
public:
    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override
    {
        if (type == BurstType::Audio) {
            ++audioCalls;
            AudioBurst audio;
            audio.index = request.index;
            audio.samples = {static_cast<int32_t>(request.index & 0x7fffffff)};
            out = audio;
            return true;
        }
        ++videoCalls;
        const int w = std::max(1, request.quality.width);
        const int h = std::max(1, request.quality.height);
        QImage img(w, h, QImage::Format_ARGB32);
        img.fill(qRgba(30, 100, 220, 128));
        out = VideoBurst{request.index, BurstQuality{w, h}, img};
        return true;
    }

    BurstOriginInfo originInfo() const override
    {
        BurstOriginInfo info;
        info.sourceVideoQuality = BurstQuality{1920, 1080};
        return info;
    }

    void invalidate(const BurstRange &range) override
    {
        Q_UNUSED(range);
    }

    int audioCalls = 0;
    int videoCalls = 0;
};

class FakeBackend : public IMediaDecoderBackend
{
public:
    bool load(const QString &path) override
    {
        loadedPath = path;
        return true;
    }

    BurstOriginInfo originInfo() const override
    {
        BurstOriginInfo info;
        info.sourceVideoQuality = BurstQuality{640, 360};
        return info;
    }

    bool decodeVideo(BurstIndex index,
                     BurstIndex decodeOrigin,
                     const BurstQuality &quality,
                     bool skipFrames,
                     VideoBurst &out) override
    {
        lastVideoDecodeOrigin = decodeOrigin;
        ++decodeVideoCalls;
        QImage img(std::max(1, quality.width), std::max(1, quality.height), QImage::Format_ARGB32);
        img.fill(skipFrames ? qRgba(0, 255, 0, 200) : qRgba(255, 0, 0, 200));
        out = VideoBurst{index, BurstQuality{img.width(), img.height()}, img};
        return true;
    }

    bool decodeAudio(BurstIndex index,
                     BurstIndex decodeOrigin,
                     int sampleRate,
                     int channels,
                     int samplesPerBurst,
                     AudioBurst &out) override
    {
        lastAudioDecodeOrigin = decodeOrigin;
        ++decodeAudioCalls;
        out.index = index;
        out.sampleRate = sampleRate;
        out.channels = channels;
        out.samples.assign(std::max(1, samplesPerBurst) * std::max(1, channels), 17);
        return true;
    }

    QString loadedPath;
    int decodeVideoCalls = 0;
    int decodeAudioCalls = 0;
    BurstIndex lastVideoDecodeOrigin = -1;
    BurstIndex lastAudioDecodeOrigin = -1;
};

void testCacheAvoidsDuplicateUpstreamCalls()
{
    auto upstream = std::make_shared<FakeBurstcon>();
    ConCacheConfig cfg;
    cfg.maxEntries = 32;
    auto cache = std::make_shared<ConCache>(upstream, QStringLiteral("test.mp4"), cfg);
    BurstRequest req{123, BurstQuality{320, 180}};
    VideoBurst v1;
    VideoBurst v2;
    assert(cache->videoBurst(req, v1));
    assert(cache->videoBurst(req, v2));
    assert(upstream->videoCalls == 1);

    AudioBurst a1;
    AudioBurst a2;
    assert(cache->audioBurst(req, a1));
    assert(cache->audioBurst(req, a2));
    assert(upstream->audioCalls == 1);
}

void testCacheInvalidationForcesRefetch()
{
    auto upstream = std::make_shared<FakeBurstcon>();
    ConCacheConfig cfg;
    auto cache = std::make_shared<ConCache>(upstream, QStringLiteral("test.mp4"), cfg);
    BurstRequest req{200, BurstQuality{640, 360}};
    VideoBurst tmp;
    (void)cache->videoBurst(req, tmp);
    (void)cache->videoBurst(req, tmp);
    assert(upstream->videoCalls == 1);
    cache->invalidate(BurstRange{180, 220});
    (void)cache->videoBurst(req, tmp);
    assert(upstream->videoCalls == 2);
}

void testConMediaUsesBackend()
{
    auto backend = std::make_shared<FakeBackend>();
    ConMedia media("fake.mp4", backend);
    BurstRequest req{42, BurstQuality{128, 72}};
    VideoBurst vb;
    AudioBurst ab;
    assert(media.videoBurst(req, vb));
    assert(media.audioBurst(req, ab));
    assert(backend->decodeVideoCalls == 1);
    assert(backend->decodeAudioCalls == 1);
}

void testConTrackAndConTrimmerComposition()
{
    ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = false;
    auto pool = std::make_shared<ConMediaPool>(poolCfg);
    auto track = std::make_shared<ConTrack>(pool);
    ConTrackEvent event;
    event.sourcePath = "fake.mp4";
    event.start = 1000;
    event.duration = 2000;
    event.sourceIn = 0;
    event.hasVideo = true;
    event.hasAudio = true;
    track->setEvents({event});

    ConTrimmer trimmer;
    trimmer.setTracks({track});
    BurstRequest req{1500, BurstQuality{160, 90}};
    VideoBurst vb;
    AudioBurst ab;
    assert(trimmer.videoBurst(req, vb));
    assert(trimmer.audioBurst(req, ab));
}

void testConTrackPropagatesDecodeOrigin()
{
    auto backend = std::make_shared<FakeBackend>();
    ConMedia media("fake.mp4", backend);
    BurstRequest req{42, BurstQuality{128, 72}, 7};
    VideoBurst vb;
    AudioBurst ab;
    assert(media.videoBurst(req, vb));
    assert(media.audioBurst(req, ab));
    assert(backend->lastVideoDecodeOrigin == 7);
    assert(backend->lastAudioDecodeOrigin == 7);
}

void testConMediaPoolReusesMediaAcrossOrigins()
{
    ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = false;
    auto pool = std::make_shared<ConMediaPool>(poolCfg);
    auto a = pool->acquire("fake.mp4", false, false, 0, true, false);
    auto b = pool->acquire("fake.mp4", false, false, 90000, true, false);
    assert(a == b);
}

void testConMediaPoolPreviewShardsAreDistinct()
{
    ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = false;
    poolCfg.previewParallelShards = 4;
    auto pool = std::make_shared<ConMediaPool>(poolCfg);
    assert(pool->previewParallelShardCount() == 4);
    auto a = pool->acquire("fake.mp4", true, false, 0, true, false, 1);
    auto b = pool->acquire("fake.mp4", true, false, 0, true, false, 2);
    assert(a != nullptr && b != nullptr);
    assert(a != b);
    auto aAgain = pool->acquire("fake.mp4", true, false, 0, true, false, 1);
    assert(aAgain == a);
}

void testOutputVideoFrameGridInvariants()
{
    assert(outputVideoFrameStepTicks(30) == 3000);
    assert(outputVideoFrameStepTicks(24) == 3750);
    assert(outputVideoFrameStepTicks(25) == 3600);
    assert(outputVideoFrameStepTicks(1) == kBurstTimebaseHz);
    assert(outputVideoFrameStepTicks(0) == kBurstTimebaseHz);

    assert(alignBurstIndexToOutputVideoFrame(30, 0) == 0);
    assert(alignBurstIndexToOutputVideoFrame(24, 3750) == 3750);
    assert(alignBurstIndexToOutputVideoFrame(24, 3749) == 0);
    assert(alignBurstIndexToOutputVideoFrame(24, 7500) == 7500);

    const BurstIndex raw = msToBurstIndex(1001);
    const BurstIndex aligned24 = alignBurstIndexToOutputVideoFrame(24, raw);
    assert(aligned24 % outputVideoFrameStepTicks(24) == 0);
    const BurstIndex manual =
        (std::max<BurstIndex>(0, raw) / outputVideoFrameStepTicks(24)) * outputVideoFrameStepTicks(24);
    assert(aligned24 == manual);

    const BurstIndex a30 = alignBurstIndexToOutputVideoFrame(30, msToBurstIndex(33));
    const BurstIndex step30 = outputVideoFrameStepTicks(30);
    const BurstIndex b30 = (msToBurstIndex(33) / step30) * step30;
    assert(a30 == b30);
}

void testMonotonicPlaybackTimerAdvances()
{
    auto upstream = std::make_shared<FakeBurstcon>();
    MonotonicPlaybackTimer timer(upstream);
    std::atomic_int videoCount{0};
    timer.setCallbacks(MonotonicPlaybackTimer::Callbacks{
        nullptr,
        [&videoCount](const VideoBurst &) { ++videoCount; },
    });
    timer.start(0);
    std::this_thread::sleep_for(std::chrono::milliseconds(35));
    timer.stop();
    assert(timer.currentIndex() > 0);
    assert(videoCount.load() > 0);
}

} // namespace

int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    testCacheAvoidsDuplicateUpstreamCalls();
    testCacheInvalidationForcesRefetch();
    testConMediaUsesBackend();
    testConTrackAndConTrimmerComposition();
    testConTrackPropagatesDecodeOrigin();
    testConMediaPoolReusesMediaAcrossOrigins();
    testConMediaPoolPreviewShardsAreDistinct();
    testOutputVideoFrameGridInvariants();
    testMonotonicPlaybackTimerAdvances();
    return 0;
}
