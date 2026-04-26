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
#include <unordered_map>

#include <QJsonArray>
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
#include "bc_fx.h"
#include "bc_internal.h"

namespace {

void crossfadePairWeights(BurstIndex t,
                          BurstIndex a0,
                          BurstIndex a1,
                          BurstIndex b0,
                          BurstIndex b1,
                          bool aFirst,
                          float &crossA,
                          float &crossB)
{
    const BurstIndex o0 = std::max(a0, b0);
    const BurstIndex o1 = std::min(a1, b1);
    if (o1 <= o0) {
        crossA = aFirst ? 1.f : 0.f;
        crossB = aFirst ? 0.f : 1.f;
        return;
    }
    const double u =
        static_cast<double>(t - o0) / static_cast<double>(std::max<BurstIndex>(1, o1 - o0));
    const double clamped = std::clamp(u, 0.0, 1.0);
    const double ang = clamped * (3.14159265358979323846 * 0.5);
    if (aFirst) {
        crossA = static_cast<float>(std::cos(ang));
        crossB = static_cast<float>(std::sin(ang));
    } else {
        crossA = static_cast<float>(std::sin(ang));
        crossB = static_cast<float>(std::cos(ang));
    }
}

} // namespace

namespace burstcon {

bool IBurstcon::audioBurst(const BurstRequest &request, AudioBurst &out)
{
    BurstPayload payload;
    if (!burst(BurstType::Audio, request, payload) || !std::holds_alternative<AudioBurst>(payload)) {
        return false;
    }
    out = std::get<AudioBurst>(std::move(payload));
    return true;
}

bool IBurstcon::videoBurst(const BurstRequest &request, VideoBurst &out)
{
    BurstPayload payload;
    if (!burst(BurstType::Video, request, payload) || !std::holds_alternative<VideoBurst>(payload)) {
        return false;
    }
    out = std::get<VideoBurst>(std::move(payload));
    return true;
}
ConMedia::ConMedia(const QString &path, std::shared_ptr<IMediaDecoderBackend> backend)
    : path(path)
    , backend(std::move(backend))
{
    if (this->backend) {
        this->backend->load(path);
    }
}

bool ConMedia::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    std::lock_guard<std::mutex> lock(backendMutex);
    if (!backend) {
        return false;
    }
    if (type == BurstType::Audio) {
        AudioBurst audioOut;
        if (!backend->decodeAudio(request.index,
                                  request.decodeOrigin,
                                  48000,
                                  2,
                                  kDefaultAudioSamplesPerBurst,
                                  audioOut)) {
            return false;
        }
        out = audioOut;
        return true;
    }
    VideoBurst videoOut;
    if (!backend->decodeVideo(request.index, request.decodeOrigin, request.quality, false, videoOut)) {
        return false;
    }
    out = videoOut;
    return true;
}

BurstOriginInfo ConMedia::originInfo() const
{
    std::lock_guard<std::mutex> lock(backendMutex);
    if (!backend) {
        return BurstOriginInfo{};
    }
    return backend->originInfo();
}

void ConMedia::syncPresentationVideoIndex(BurstIndex mediaBurstIndex)
{
    std::lock_guard<std::mutex> lock(backendMutex);
    if (backend) {
        backend->setPresentationVideoIndex(mediaBurstIndex);
    }
}

void ConMedia::invalidate(const BurstRange &range)
{
    Q_UNUSED(range);
}
ConTrack::ConTrack(std::shared_ptr<ConMediaPool> mediaPool)
    : mediaPool(std::move(mediaPool))
{
}

void ConTrack::setOutputVideoSize(BurstQuality q)
{
    std::lock_guard<std::mutex> lock(mutex);
    outputVideoSize = q;
}

void ConTrack::setEvents(const std::vector<ConTrackEvent> &events)
{
    BurstQuality outVideoSizeCopy;
    {
        std::lock_guard<std::mutex> lock(mutex);
        outVideoSizeCopy = outputVideoSize;
    }
    std::vector<BoundEvent> bound;
    bound.reserve(events.size());
    for (const auto &event : events) {
        BoundEvent b;
        b.event = event;
        b.media = mediaPool ? mediaPool->acquire(event.sourcePath,
                                                 false,
                                                 false,
                                                 event.sourceIn,
                                                 event.hasVideo,
                                                 event.hasAudio)
                            : nullptr;
        if (b.media) {
            if (event.hasVideo) {
                QJsonArray vfx = event.videoFxChain;
                if (vfx.isEmpty()) {
                    vfx = defaultVideoFxChain();
                }
                b.media = wrapVideoFxChain(std::move(b.media), vfx, outVideoSizeCopy);
            }
            if (event.hasAudio) {
                b.media = wrapAudioFxChain(std::move(b.media), event.audioFxChain);
            }
        }
        bound.push_back(b);
    }
    std::sort(bound.begin(), bound.end(), [](const BoundEvent &a, const BoundEvent &b) {
        if (a.event.start != b.event.start) {
            return a.event.start < b.event.start;
        }
        return a.event.zIndex < b.event.zIndex;
    });
    std::lock_guard<std::mutex> lock(mutex);
    this->events = std::move(bound);
}

bool ConTrack::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    std::vector<BoundEvent> local;
    {
        std::lock_guard<std::mutex> lock(mutex);
        local = events;
    }
    if (type == BurstType::Audio) {
        struct Active {
            size_t boundIndex = 0;
            AudioBurst burst;
        };
        std::vector<Active> active;
        active.reserve(local.size());
        for (size_t bi = 0; bi < local.size(); ++bi) {
            const auto &bound = local[bi];
            const auto &event = bound.event;
            if (!event.hasAudio || !bound.media) {
                continue;
            }
            if (request.index < event.start || request.index >= event.start + event.duration) {
                continue;
            }
            AudioBurst burst;
            const BurstIndex localIndex = event.sourceIn + (request.index - event.start);
            if (!bound.media->audioBurst(BurstRequest{localIndex, request.quality, event.sourceIn}, burst)) {
                continue;
            }
            active.push_back(Active{bi, std::move(burst)});
        }
        if (active.empty()) {
            return false;
        }

        std::unordered_map<int, std::vector<size_t>> byLane;
        byLane.reserve(active.size());
        for (size_t ai = 0; ai < active.size(); ++ai) {
            const int lane = local[active[ai].boundIndex].event.timelineLaneIndex;
            byLane[lane].push_back(ai);
        }

        std::vector<float> weight(active.size(), 1.f);
        for (const auto &kv : byLane) {
            const std::vector<size_t> &inds = kv.second;
            if (inds.size() <= 1) {
                continue;
            }
            if (inds.size() == 2) {
                const auto &e0 = local[active[inds[0]].boundIndex].event;
                const auto &e1 = local[active[inds[1]].boundIndex].event;
                float c0 = 1.f;
                float c1 = 1.f;
                crossfadePairWeights(request.index,
                                     e0.start,
                                     e0.start + e0.duration,
                                     e1.start,
                                     e1.start + e1.duration,
                                     e0.start <= e1.start,
                                     c0,
                                     c1);
                weight[inds[0]] = c0;
                weight[inds[1]] = c1;
            } else {
                const float inv = 1.f / static_cast<float>(inds.size());
                for (size_t k : inds) {
                    weight[k] = inv;
                }
            }
        }

        for (size_t ai = 0; ai < active.size(); ++ai) {
            weight[ai] *= std::max(0.f, local[active[ai].boundIndex].event.audioLinearGain);
        }

        AudioBurst base = active[0].burst;
        std::vector<int64_t> mix(base.samples.size(), 0);
        for (size_t ai = 0; ai < active.size(); ++ai) {
            const AudioBurst &burst = active[ai].burst;
            if (mix.size() < burst.samples.size()) {
                mix.resize(burst.samples.size(), 0);
                base.samples.resize(burst.samples.size(), 0);
            }
            const float w = weight[ai];
            for (size_t i = 0; i < burst.samples.size(); ++i) {
                mix[i] += static_cast<int64_t>(std::llround(static_cast<double>(burst.samples[i]) * static_cast<double>(w)));
            }
        }
        for (size_t i = 0; i < mix.size(); ++i) {
            base.samples[i] = static_cast<int32_t>(std::clamp<int64_t>(
                mix[i],
                std::numeric_limits<int32_t>::min(),
                std::numeric_limits<int32_t>::max()));
        }
        base.index = request.index;
        out = std::move(base);
        return true;
    }

    struct VideoLayerCandidate {
        int zIndex = 0;
        BurstIndex start = 0;
        QString path;
        VideoBurst burst;
    };
    std::vector<VideoLayerCandidate> layers;
    for (const auto &bound : local) {
        const auto &event = bound.event;
        if (!event.hasVideo || !bound.media) {
            continue;
        }
        if (request.index < event.start || request.index >= event.start + event.duration) {
            continue;
        }
        VideoBurst burst;
        const BurstIndex localIndex = event.sourceIn + (request.index - event.start);
        if (!bound.media->videoBurst(BurstRequest{localIndex, request.quality, event.sourceIn}, burst)) {
            continue;
        }
        layers.push_back(VideoLayerCandidate{event.zIndex, event.start, event.sourcePath, std::move(burst)});
    }
    if (layers.empty()) {
        return false;
    }
    std::sort(layers.begin(), layers.end(), [](const VideoLayerCandidate &a, const VideoLayerCandidate &b) {
        if (a.zIndex != b.zIndex) {
            return a.zIndex < b.zIndex;
        }
        if (a.start != b.start) {
            return a.start < b.start;
        }
        return a.path < b.path;
    });
    if (layers.size() == 1) {
        layers[0].burst.index = request.index;
        out = std::move(layers[0].burst);
        return true;
    }
    QImage composed = layers[0].burst.frame.convertToFormat(QImage::Format_ARGB32);
    for (size_t i = 1; i < layers.size(); ++i) {
        QPainter painter(&composed);
        painter.setCompositionMode(QPainter::CompositionMode_SourceOver);
        painter.drawImage(composed.rect(), layers[i].burst.frame);
    }
    out = VideoBurst{request.index, BurstQuality{composed.width(), composed.height()}, std::move(composed)};
    return true;
}

BurstOriginInfo ConTrack::originInfo() const
{
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto &bound : events) {
        if (bound.media) {
            return bound.media->originInfo();
        }
    }
    return BurstOriginInfo{};
}

void ConTrack::invalidate(const BurstRange &range)
{
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto &bound : events) {
        if (bound.media) {
            bound.media->invalidate(range);
        }
    }
}

void ConTrimmer::setTracks(const std::vector<std::shared_ptr<ConTrack>> &tracks)
{
    std::lock_guard<std::mutex> lock(mutex);
    this->tracks = tracks;
}

bool ConTrimmer::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    std::vector<std::shared_ptr<ConTrack>> localTracks;
    {
        std::lock_guard<std::mutex> lock(mutex);
        localTracks = tracks;
    }
    if (type == BurstType::Audio) {
        std::vector<int64_t> mix;
        AudioBurst base;
        bool hasAny = false;
        for (const auto &track : localTracks) {
            if (!track) {
                continue;
            }
            AudioBurst burst;
            if (!track->audioBurst(request, burst)) {
                continue;
            }
            if (!hasAny) {
                base = burst;
                mix.assign(base.samples.size(), 0);
                hasAny = true;
            }
            if (mix.size() < burst.samples.size()) {
                mix.resize(burst.samples.size(), 0);
                base.samples.resize(burst.samples.size(), 0);
            }
            for (size_t i = 0; i < burst.samples.size(); ++i) {
                mix[i] += burst.samples[i];
            }
        }
        if (!hasAny) {
            return false;
        }
        for (size_t i = 0; i < mix.size(); ++i) {
            base.samples[i] = static_cast<int32_t>(std::clamp<int64_t>(
                mix[i],
                std::numeric_limits<int32_t>::min(),
                std::numeric_limits<int32_t>::max()));
        }
        base.index = request.index;
        out = base;
        return true;
    }

    std::vector<VideoBurst> videoLayers;
    videoLayers.reserve(localTracks.size());
    for (const auto &track : localTracks) {
        if (!track) {
            continue;
        }
        VideoBurst burst;
        if (!track->videoBurst(request, burst) || burst.frame.isNull()) {
            continue;
        }
        videoLayers.push_back(std::move(burst));
    }
    if (videoLayers.empty()) {
        return false;
    }
    if (videoLayers.size() == 1) {
        videoLayers[0].index = request.index;
        out = std::move(videoLayers[0]);
        return true;
    }
    QImage composed = videoLayers[0].frame.convertToFormat(QImage::Format_ARGB32);
    for (size_t i = 1; i < videoLayers.size(); ++i) {
        QPainter painter(&composed);
        painter.setCompositionMode(QPainter::CompositionMode_SourceOver);
        painter.drawImage(composed.rect(), videoLayers[i].frame);
    }
    out = VideoBurst{request.index, BurstQuality{composed.width(), composed.height()}, std::move(composed)};
    return true;
}

BurstOriginInfo ConTrimmer::originInfo() const
{
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto &track : tracks) {
        if (track) {
            const BurstOriginInfo info = track->originInfo();
            if (info.sourceVideoQuality.width > 0 && info.sourceVideoQuality.height > 0) {
                return info;
            }
        }
    }
    return BurstOriginInfo{};
}

void ConTrimmer::invalidate(const BurstRange &range)
{
    std::vector<std::shared_ptr<ConTrack>> localTracks;
    {
        std::lock_guard<std::mutex> lock(mutex);
        localTracks = tracks;
    }
    for (const auto &track : localTracks) {
        if (track) {
            track->invalidate(range);
        }
    }
}

} // namespace burstcon
