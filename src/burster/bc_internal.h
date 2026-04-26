#ifndef BC_INTERNAL_H
#define BC_INTERNAL_H

#include "bc.h"

#include <QImage>
#include <QString>

#include <atomic>
#include <cstdint>
#include <memory>

namespace burstcon {

struct SharedMediaFile;

extern std::atomic<uint64_t> gAvcppOpenCalls;
extern std::atomic<uint64_t> gAvcppPacketReads;
extern std::atomic<uint64_t> gAvcppVideoDecodeCalls;
extern std::atomic<uint64_t> gAvcppAudioDecodeCalls;
extern std::atomic<uint64_t> gCacheHits;
extern std::atomic<uint64_t> gCacheMisses;
extern std::atomic<int64_t> gCacheEntries;
extern std::atomic<uint64_t> gCacheVideoUnusedEvictions;

constexpr int kDefaultAudioSamplesPerBurst = 1024;

std::shared_ptr<IMediaDecoderBackend> makeAvcppMediaDecoderBackend(std::shared_ptr<SharedMediaFile> sharedFile,
                                                                  bool skipFrames,
                                                                  bool wantVideo,
                                                                  bool wantAudio);

inline int64_t qualityArea(const BurstQuality &q)
{
    if (q.width <= 0 || q.height <= 0) {
        return 0;
    }
    return static_cast<int64_t>(q.width) * static_cast<int64_t>(q.height);
}

inline uint64_t imageBytes(const QImage &img)
{
#if QT_VERSION >= QT_VERSION_CHECK(5, 10, 0)
    return static_cast<uint64_t>(img.sizeInBytes());
#else
    return static_cast<uint64_t>(img.byteCount());
#endif
}

} // namespace burstcon

#endif // BC_INTERNAL_H
