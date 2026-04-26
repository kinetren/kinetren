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

size_t ConCache::AudioKeyHash::operator()(const AudioKey &key) const
{
    return std::hash<int64_t>{}(key.index);
}

size_t ConCache::VideoKeyHash::operator()(const VideoKey &key) const
{
    size_t h = std::hash<int64_t>{}(key.index);
    h ^= static_cast<size_t>(key.width + 0x9e3779b9U + (h << 6U) + (h >> 2U));
    h ^= static_cast<size_t>(key.height + 0x9e3779b9U + (h << 6U) + (h >> 2U));
    return h;
}

void ConCache::notifyVideoFrameCache(bool inserted, const VideoKey &key)
{
    if (!config.onVideoFrameCacheChanged || mediaPath.isEmpty()) {
        return;
    }
    config.onVideoFrameCacheChanged(inserted, mediaPath, key.index, BurstQuality{key.width, key.height});
}

ConCache::ConCache(std::shared_ptr<IBurstcon> upstream, QString mediaPath, const ConCacheConfig &config)
    : upstream(std::move(upstream))
    , mediaPath(std::move(mediaPath))
    , config(config)
{
    if (this->config.preloadEnabled) {
        preloadThread = std::thread([this]() { preloadLoop(); });
    }
}

ConCache::~ConCache()
{
    stopPreload.store(true);
    preloadCv.notify_all();
    if (preloadThread.joinable()) {
        preloadThread.join();
    }
}

bool ConCache::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    if (type == BurstType::Audio) {
        AudioBurst audioOut;
        const AudioKey key = keyForAudio(request);
        if (getAudio(key, audioOut)) {
            out = audioOut;
            return true;
        }
        if (!upstream->audioBurst(request, audioOut)) {
            return false;
        }
        put(key, audioOut, request.index);
        out = audioOut;
        return true;
    }

    BurstRequest effective = request;
    effective.quality = effectiveVideoQuality(request);
    VideoBurst videoOut;
    const VideoKey key = keyForVideo(effective);
    if (getVideo(key, videoOut)) {
        // Cache hit does not go through `put`, so listeners (e.g. timeline coverage stripes /
        // `isPlaybackHeadWarm`) never see the frame unless we notify here. After a graph rebuild
        // clears bucket sets, hits must republish composition coverage.
        notifyVideoFrameCache(true, key);
        out = videoOut;
        return true;
    }
    if (!upstream->videoBurst(effective, videoOut)) {
        static std::atomic<uint64_t> sConCacheVideoFailN{0};
        const uint64_t n = ++sConCacheVideoFailN;
        const BurstOriginInfo oi = upstream->originInfo();
        if ((n % 8u) == 1u) {
            reportBurstconDiagnostic(
                QStringLiteral("ConCache upstream videoBurst failed n=%1 media=%2 idx=%3 "
                              "reqQ=%4x%5 effQ=%6x%7 keyIdx=%8 keyQ=%9x%10 fileSrc=%11x%12 frameNull=%13 | "
                              "forcedQOn=%14 forcedArea=%15 (see 'video decode exhausted' on same file)")
                    .arg(n)
                    .arg(mediaPath)
                    .arg(effective.index)
                    .arg(request.quality.width)
                    .arg(request.quality.height)
                    .arg(effective.quality.width)
                    .arg(effective.quality.height)
                    .arg(key.index)
                    .arg(key.width)
                    .arg(key.height)
                    .arg(oi.sourceVideoQuality.width)
                    .arg(oi.sourceVideoQuality.height)
                    .arg(videoOut.frame.isNull() ? 1 : 0)
                    .arg(config.forcedVideoQualityEnabled ? 1 : 0)
                    .arg(qualityArea(config.forcedVideoQuality)));
        }
        return false;
    }
    put(key, videoOut, effective.index);
    out = videoOut;
    return true;
}

BurstOriginInfo ConCache::originInfo() const
{
    return upstream->originInfo();
}

void ConCache::syncPresentationVideoIndex(BurstIndex mediaBurstIndex)
{
    if (upstream) {
        upstream->syncPresentationVideoIndex(mediaBurstIndex);
    }
}

void ConCache::invalidate(const BurstRange &range)
{
    std::vector<VideoKey> videoRemoved;
    {
        upstream->invalidate(range);
        std::lock_guard<std::mutex> lock(mutex);
        for (auto it = audioEntries.begin(); it != audioEntries.end();) {
            if (it->second.index >= range.start && it->second.index <= range.end) {
                audioLru.erase(it->second.lruIt);
                gCacheEntries.fetch_sub(1);
                it = audioEntries.erase(it);
            } else {
                ++it;
            }
        }
        for (auto it = videoEntries.begin(); it != videoEntries.end();) {
            if (it->second.index >= range.start && it->second.index <= range.end) {
                if (!it->second.hadHit) {
                    gCacheVideoUnusedEvictions.fetch_add(1);
                }
                videoRemoved.push_back(it->first);
                videoLru.erase(it->second.lruIt);
                videoBytesUsed -= std::min<uint64_t>(videoBytesUsed, imageBytes(it->second.value.frame));
                gCacheEntries.fetch_sub(1);
                it = videoEntries.erase(it);
            } else {
                ++it;
            }
        }
    }
    for (const auto &vk : videoRemoved) {
        notifyVideoFrameCache(false, vk);
    }
}

bool ConCache::getAudio(const AudioKey &key, AudioBurst &out)
{
    std::lock_guard<std::mutex> lock(mutex);
    auto it = audioEntries.find(key);
    if (it == audioEntries.end()) {
        gCacheMisses.fetch_add(1);
        return false;
    }
    gCacheHits.fetch_add(1);
    touchAudioLocked(key, it->second);
    out = it->second.value;
    return true;
}

bool ConCache::getVideo(const VideoKey &key, VideoBurst &out)
{
    std::unique_lock<std::mutex> lock(mutex);
    auto it = videoEntries.find(key);
    if (it == videoEntries.end()) {
        gCacheMisses.fetch_add(1);
        return false;
    }
    gCacheHits.fetch_add(1);
    it->second.hadHit = true;
    touchVideoLocked(key, it->second);
    out = it->second.value;
    lock.unlock();
    if (upstream) {
        upstream->syncPresentationVideoIndex(key.index);
    }
    return true;
}

void ConCache::put(const AudioKey &key, const AudioBurst &value, BurstIndex index)
{
    std::lock_guard<std::mutex> lock(mutex);
    auto existing = audioEntries.find(key);
    if (existing != audioEntries.end()) {
        audioLru.erase(existing->second.lruIt);
        audioEntries.erase(existing);
        gCacheEntries.fetch_sub(1);
    }
    audioLru.push_front(key);
    audioEntries[key] = AudioEntry{value, audioLru.begin(), index};
    gCacheEntries.fetch_add(1);
    evictAudioLocked();
}

void ConCache::put(const VideoKey &key, const VideoBurst &value, BurstIndex index)
{
    std::vector<VideoKey> removedVideoKeys;
    {
        std::lock_guard<std::mutex> lock(mutex);
        auto existing = videoEntries.find(key);
        if (existing != videoEntries.end()) {
            if (!existing->second.hadHit) {
                gCacheVideoUnusedEvictions.fetch_add(1);
            }
            removedVideoKeys.push_back(existing->first);
            videoBytesUsed -= std::min<uint64_t>(videoBytesUsed, imageBytes(existing->second.value.frame));
            videoLru.erase(existing->second.lruIt);
            videoEntries.erase(existing);
            gCacheEntries.fetch_sub(1);
        }
        videoLru.push_front(key);
        videoEntries[key] = VideoEntry{value, videoLru.begin(), index, false};
        videoBytesUsed += imageBytes(value.frame);
        gCacheEntries.fetch_add(1);
        evictVideoLocked(removedVideoKeys);
    }
    for (const auto &vk : removedVideoKeys) {
        notifyVideoFrameCache(false, vk);
    }
    notifyVideoFrameCache(true, key);
}

void ConCache::evictAudioLocked()
{
    while (((audioEntries.size() + videoEntries.size()) > config.maxEntries || audioEntries.size() > config.maxAudioEntries) &&
           !audioLru.empty()) {
        const AudioKey key = audioLru.back();
        audioLru.pop_back();
        audioEntries.erase(key);
        gCacheEntries.fetch_sub(1);
    }
}

void ConCache::evictVideoLocked(std::vector<VideoKey> &removedVideoKeys)
{
    while (((audioEntries.size() + videoEntries.size()) > config.maxEntries ||
            videoEntries.size() > config.maxVideoEntries ||
            videoBytesUsed > config.maxVideoBytes) &&
           !videoLru.empty()) {
        const VideoKey key = videoLru.back();
        auto it = videoEntries.find(key);
        if (it != videoEntries.end()) {
            if (!it->second.hadHit) {
                gCacheVideoUnusedEvictions.fetch_add(1);
            }
            removedVideoKeys.push_back(key);
            videoBytesUsed -= std::min<uint64_t>(videoBytesUsed, imageBytes(it->second.value.frame));
            videoEntries.erase(it);
            gCacheEntries.fetch_sub(1);
        }
        videoLru.pop_back();
    }
}

void ConCache::touchAudioLocked(const AudioKey &key, AudioEntry &entry)
{
    audioLru.erase(entry.lruIt);
    audioLru.push_front(key);
    entry.lruIt = audioLru.begin();
}

void ConCache::touchVideoLocked(const VideoKey &key, VideoEntry &entry)
{
    videoLru.erase(entry.lruIt);
    videoLru.push_front(key);
    entry.lruIt = videoLru.begin();
}

ConCache::AudioKey ConCache::keyForAudio(const BurstRequest &request) const
{
    AudioKey key;
    key.index = request.index;
    return key;
}

ConCache::VideoKey ConCache::keyForVideo(const BurstRequest &request) const
{
    VideoKey key;
    key.index = request.index;
    key.width = request.quality.width;
    key.height = request.quality.height;
    return key;
}

BurstQuality ConCache::effectiveVideoQuality(const BurstRequest &request) const
{
    BurstQuality effective = request.quality;
    const BurstOriginInfo info = upstream->originInfo();
    if (effective.width <= 0 || effective.height <= 0) {
        effective = info.sourceVideoQuality;
    }
    if (config.forcedVideoQualityEnabled && qualityArea(config.forcedVideoQuality) > 0) {
        const BurstQuality forced = config.forcedVideoQuality;
        if (qualityArea(forced) < qualityArea(info.sourceVideoQuality)) {
            effective = forced;
        } else if (qualityArea(effective) < qualityArea(info.sourceVideoQuality)) {
            effective = info.sourceVideoQuality;
        }
    }
    return effective;
}

void ConCache::schedulePreload(const BurstRequest &request, bool video, bool internalRequest)
{
    Q_UNUSED(request);
    Q_UNUSED(video);
    Q_UNUSED(internalRequest);
}

void ConCache::preloadLoop()
{
}

void ConCache::preloadBurst(BurstRequest request, bool video)
{
    Q_UNUSED(request);
    Q_UNUSED(video);
}

} // namespace burstcon
