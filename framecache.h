#ifndef FRAMECACHE_H
#define FRAMECACHE_H

#include <QImage>
#include <QtGlobal>

#include <list>
#include <mutex>
#include <string>
#include <unordered_map>

class FrameCache
{
public:
    struct Stats {
        quint64 entries = 0;
        quint64 usedBytes = 0;
        quint64 limitBytes = 0;
        quint64 configuredMaxBytes = 0;
        quint64 hits = 0;
        quint64 misses = 0;
        quint64 evictions = 0;
    };

    FrameCache();

    void setConfiguredMaxBytes(quint64 bytes);
    void clear();
    bool get(const std::string &key, QImage &outImage);
    bool contains(const std::string &key);
    void put(const std::string &key, const QImage &image);
    Stats stats() const;

private:
    struct Entry {
        QImage image;
        quint64 bytes = 0;
        std::list<std::string>::iterator lruIt;
    };

    quint64 effectiveLimitBytesLocked() const;
    void evictIfNeededLocked();
    static quint64 totalRamBytes();
    static quint64 freeRamBytes();
    static quint64 imageBytes(const QImage &image);

    mutable std::mutex mutex;
    std::unordered_map<std::string, Entry> entries;
    std::list<std::string> lru;
    quint64 usedBytes = 0;
    quint64 configuredMaxBytes = 0;
    quint64 hitCount = 0;
    quint64 missCount = 0;
    quint64 evictionCount = 0;
};

#endif // FRAMECACHE_H
