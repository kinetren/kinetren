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

void ConTrack::setEvents(const std::vector<ConTrackEvent> &events)
{
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
        std::vector<int64_t> mix;
        AudioBurst base;
        bool hasAny = false;
        for (const auto &bound : local) {
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
