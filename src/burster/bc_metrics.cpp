#include "bc.h"
#include "bc_internal.h"

#include <algorithm>
#include <functional>
#include <mutex>

namespace burstcon {

std::atomic<uint64_t> gAvcppOpenCalls{0};
std::atomic<uint64_t> gAvcppPacketReads{0};
std::atomic<uint64_t> gAvcppVideoDecodeCalls{0};
std::atomic<uint64_t> gAvcppAudioDecodeCalls{0};
std::atomic<uint64_t> gCacheHits{0};
std::atomic<uint64_t> gCacheMisses{0};
std::atomic<int64_t> gCacheEntries{0};
std::atomic<uint64_t> gCacheVideoUnusedEvictions{0};

BurstconMetrics metricsSnapshot()
{
    BurstconMetrics m;
    m.avcppOpenCalls = gAvcppOpenCalls.load();
    m.avcppPacketReads = gAvcppPacketReads.load();
    m.avcppVideoDecodeCalls = gAvcppVideoDecodeCalls.load();
    m.avcppAudioDecodeCalls = gAvcppAudioDecodeCalls.load();
    m.cacheHits = gCacheHits.load();
    m.cacheMisses = gCacheMisses.load();
    m.cacheEntries = static_cast<uint64_t>(std::max<int64_t>(0, gCacheEntries.load()));
    m.cacheVideoUnusedEvictions = gCacheVideoUnusedEvictions.load();
    return m;
}

void resetMetrics()
{
    gAvcppOpenCalls.store(0);
    gAvcppPacketReads.store(0);
    gAvcppVideoDecodeCalls.store(0);
    gAvcppAudioDecodeCalls.store(0);
    gCacheHits.store(0);
    gCacheMisses.store(0);
    gCacheEntries.store(0);
    gCacheVideoUnusedEvictions.store(0);
}

static std::mutex gDiagnosticMutex;
static std::function<void(const QString &)> gDiagnosticHandler;

void setBurstconDiagnosticHandler(std::function<void(const QString &)> handler)
{
    std::lock_guard<std::mutex> lock(gDiagnosticMutex);
    gDiagnosticHandler = std::move(handler);
}

void reportBurstconDiagnostic(const QString &message)
{
    std::function<void(const QString &)> cb;
    {
        std::lock_guard<std::mutex> lock(gDiagnosticMutex);
        cb = gDiagnosticHandler;
    }
    if (cb) {
        cb(message);
    }
}

void reportBurstconDiagnosticEvery(const QString &message, std::atomic<uint64_t> &which,
                                   uint32_t nMod /* 1=always, 48=like (n%48)==1 */)
{
    if (nMod == 0) {
        return;
    }
    const uint64_t k = which.fetch_add(1) + 1;
    if (nMod == 1u || (k % static_cast<uint64_t>(nMod)) == 1u) {
        reportBurstconDiagnostic(message);
    }
}

} // namespace burstcon
