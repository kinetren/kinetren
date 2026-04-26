#include "bc.h"

#include <algorithm>
#include <cstdint>

namespace burstcon {

BurstIndex outputVideoFrameStepTicks(int outputFps)
{
    const int fps = std::max(1, outputFps);
    return std::max<BurstIndex>(1, kBurstTimebaseHz / static_cast<BurstIndex>(fps));
}

BurstIndex alignBurstIndexToOutputVideoFrame(int outputFps, BurstIndex index)
{
    const BurstIndex step = outputVideoFrameStepTicks(outputFps);
    const BurstIndex safe = std::max<BurstIndex>(0, index);
    return (safe / step) * step;
}

int64_t msToBurstIndex(int64_t ms)
{
    if (ms <= 0) {
        return 0;
    }
    return (ms * kBurstTimebaseHz) / 1000;
}

BurstIndex msToBurstIndexCeil(int64_t ms)
{
    const int64_t safe = std::max<int64_t>(1, ms);
    return static_cast<BurstIndex>((safe * kBurstTimebaseHz + 999) / 1000);
}

int64_t burstIndexToMs(BurstIndex index)
{
    if (index <= 0) {
        return 0;
    }
    return (index * 1000) / kBurstTimebaseHz;
}

} // namespace burstcon
