#include "bc.h"
#include "bc_internal.h"

#include <algorithm>
#include <memory>
#include <string>

namespace burstcon {

ConMediaPool::ConMediaPool(const ConMediaPoolConfig &config)
    : config(config)
{
}

size_t ConMediaPool::previewParallelShardCount() const
{
    return std::max<size_t>(1, config.previewParallelShards);
}

std::shared_ptr<IBurstcon> ConMediaPool::acquire(const QString &path,
                                                 bool forPreview,
                                                 bool skipFrames,
                                                 BurstIndex decodeOrigin,
                                                 bool wantVideo,
                                                 bool wantAudio,
                                                 size_t poolKeyVariant)
{
    Q_UNUSED(decodeOrigin);
    std::string key = (forPreview ? "p:" : "m:") + path.toStdString() + (skipFrames ? ":1" : ":0") + ":" +
        (wantVideo ? "v" : "-") + (wantAudio ? "a" : "-");
    if (poolKeyVariant != 0) {
        key += ":s:";
        key += std::to_string(poolKeyVariant);
    }
    std::lock_guard<std::mutex> lock(mutex);
    auto it = mediaByKey.find(key);
    if (it != mediaByKey.end()) {
        mediaLru.remove(key);
        mediaLru.push_front(key);
        return it->second;
    }
    auto created = createMedia(path, forPreview, skipFrames, -1, wantVideo, wantAudio);
    mediaByKey[key] = created;
    mediaLru.push_front(key);
    while (mediaByKey.size() > config.maxMediaInstances && !mediaLru.empty()) {
        const std::string evict = mediaLru.back();
        mediaLru.pop_back();
        mediaByKey.erase(evict);
    }
    return created;
}

void ConMediaPool::invalidatePath(const QString &path)
{
    std::lock_guard<std::mutex> lock(mutex);
    if (path.isEmpty()) {
        mediaByKey.clear();
        sharedFilesByPath.clear();
        mediaLru.clear();
        return;
    }
    for (auto it = mediaByKey.begin(); it != mediaByKey.end();) {
        if (it->first.find(path.toStdString()) != std::string::npos) {
            mediaLru.remove(it->first);
            it = mediaByKey.erase(it);
        } else {
            ++it;
        }
    }
    for (auto it = sharedFilesByPath.begin(); it != sharedFilesByPath.end();) {
        if (it->first.find(path.toStdString()) != std::string::npos) {
            it = sharedFilesByPath.erase(it);
        } else {
            ++it;
        }
    }
}

std::shared_ptr<IBurstcon> ConMediaPool::createMedia(const QString &path,
                                                     bool forPreview,
                                                     bool skipFrames,
                                                     BurstIndex decodeOrigin,
                                                     bool wantVideo,
                                                     bool wantAudio)
{
    Q_UNUSED(decodeOrigin);
    auto backend = makeAvcppMediaDecoderBackend(acquireSharedMediaFile(path), skipFrames, wantVideo, wantAudio);
    std::shared_ptr<IBurstcon> media = std::make_shared<ConMedia>(path, backend);
    if (config.enableConMediaCache) {
        ConCacheConfig c = config.cacheConfig;
        if (forPreview && config.forcedPreviewQualityEnabled) {
            c.forcedVideoQualityEnabled = true;
            c.forcedVideoQuality = config.forcedPreviewQuality;
        }
        media = std::make_shared<ConCache>(media, path, c);
    }
    return media;
}

} // namespace burstcon
