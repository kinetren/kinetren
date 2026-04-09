#include "framecache.h"

#include <algorithm>

#ifdef __linux__
#include <sys/sysinfo.h>
#endif

namespace {
constexpr quint64 kTwoGiB = 2ULL * 1024ULL * 1024ULL * 1024ULL;
}

FrameCache::FrameCache()
{
    const quint64 ramTotal = totalRamBytes();
    configuredMaxBytes = std::max<quint64>(kTwoGiB, ramTotal / 8ULL);
}

void FrameCache::setConfiguredMaxBytes(quint64 bytes)
{
    std::lock_guard<std::mutex> lock(mutex);
    configuredMaxBytes = std::max<quint64>(1, bytes);
    evictIfNeededLocked();
}

void FrameCache::clear()
{
    std::lock_guard<std::mutex> lock(mutex);
    entries.clear();
    lru.clear();
    usedBytes = 0;
}

bool FrameCache::get(const std::string &key, QImage &outImage)
{
    std::lock_guard<std::mutex> lock(mutex);
    auto it = entries.find(key);
    if (it == entries.end()) {
        ++missCount;
        return false;
    }
    lru.erase(it->second.lruIt);
    lru.push_front(key);
    it->second.lruIt = lru.begin();
    outImage = it->second.image;
    ++hitCount;
    return true;
}

bool FrameCache::contains(const std::string &key)
{
    std::lock_guard<std::mutex> lock(mutex);
    auto it = entries.find(key);
    if (it == entries.end()) {
        ++missCount;
        return false;
    }
    lru.erase(it->second.lruIt);
    lru.push_front(key);
    it->second.lruIt = lru.begin();
    ++hitCount;
    return true;
}

void FrameCache::put(const std::string &key, const QImage &image)
{
    if (image.isNull()) {
        return;
    }
    const quint64 bytes = imageBytes(image);
    if (bytes == 0) {
        return;
    }

    std::lock_guard<std::mutex> lock(mutex);
    const quint64 limit = effectiveLimitBytesLocked();
    if (bytes > limit) {
        return;
    }

    auto existing = entries.find(key);
    if (existing != entries.end()) {
        usedBytes -= existing->second.bytes;
        lru.erase(existing->second.lruIt);
        entries.erase(existing);
    }

    lru.push_front(key);
    entries.emplace(key, Entry{image, bytes, lru.begin()});
    usedBytes += bytes;

    evictIfNeededLocked();
}

FrameCache::Stats FrameCache::stats() const
{
    std::lock_guard<std::mutex> lock(mutex);
    Stats s;
    s.entries = static_cast<quint64>(entries.size());
    s.usedBytes = usedBytes;
    s.limitBytes = effectiveLimitBytesLocked();
    s.configuredMaxBytes = configuredMaxBytes;
    s.hits = hitCount;
    s.misses = missCount;
    s.evictions = evictionCount;
    return s;
}

quint64 FrameCache::effectiveLimitBytesLocked() const
{
    const quint64 freeBytes = freeRamBytes();
    if (freeBytes == 0) {
        return configuredMaxBytes;
    }
    return std::min(configuredMaxBytes, freeBytes);
}

void FrameCache::evictIfNeededLocked()
{
    const quint64 limit = effectiveLimitBytesLocked();
    while (usedBytes > limit && !lru.empty()) {
        const std::string key = lru.back();
        lru.pop_back();
        auto it = entries.find(key);
        if (it == entries.end()) {
            continue;
        }
        usedBytes -= it->second.bytes;
        entries.erase(it);
        ++evictionCount;
    }
}

quint64 FrameCache::totalRamBytes()
{
#ifdef __linux__
    struct sysinfo info {};
    if (sysinfo(&info) != 0) {
        return 8ULL * 1024ULL * 1024ULL * 1024ULL;
    }
    return static_cast<quint64>(info.totalram) * static_cast<quint64>(info.mem_unit);
#else
    return 8ULL * 1024ULL * 1024ULL * 1024ULL;
#endif
}

quint64 FrameCache::freeRamBytes()
{
#ifdef __linux__
    struct sysinfo info {};
    if (sysinfo(&info) != 0) {
        return 0;
    }
    return static_cast<quint64>(info.freeram) * static_cast<quint64>(info.mem_unit);
#else
    return 0;
#endif
}

quint64 FrameCache::imageBytes(const QImage &image)
{
#if QT_VERSION >= QT_VERSION_CHECK(5, 10, 0)
    return static_cast<quint64>(image.sizeInBytes());
#else
    return static_cast<quint64>(image.byteCount());
#endif
}
