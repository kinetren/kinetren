#include "projectsettings.h"

#include <algorithm>
#include <cmath>

double ProjectSettings::cacheScaleFactor() const
{
    switch (cacheQuality) {
    case CacheQuality::Quarter16:
        return 1.0 / 16.0;
    case CacheQuality::Quarter8:
        return 1.0 / 8.0;
    case CacheQuality::Quarter4:
        return 1.0 / 4.0;
    case CacheQuality::Half:
        return 1.0 / 2.0;
    case CacheQuality::Source:
    default:
        return 1.0;
    }
}

QString ProjectSettings::cacheQualityLabel() const
{
    switch (cacheQuality) {
    case CacheQuality::Quarter16:
        return "1/16";
    case CacheQuality::Quarter8:
        return "1/8";
    case CacheQuality::Quarter4:
        return "1/4";
    case CacheQuality::Half:
        return "1/2";
    case CacheQuality::Source:
    default:
        return "1/1 (src)";
    }
}

QSize ProjectSettings::cacheResolution() const
{
    const double scale = cacheScaleFactor();
    return QSize(std::max(1, static_cast<int>(resolution.width() * scale)),
                 std::max(1, static_cast<int>(resolution.height() * scale)));
}

int ProjectSettings::effectiveDecodeSeekLookbackMs() const
{
    if (decodeSeekLookbackMs > 0) {
        return decodeSeekLookbackMs;
    }
    const int safeFps = std::max(1, fps);
    return std::max(1, static_cast<int>(std::lround(1000.0 / static_cast<double>(safeFps))));
}
