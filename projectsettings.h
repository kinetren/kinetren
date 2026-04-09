#ifndef PROJECTSETTINGS_H
#define PROJECTSETTINGS_H

#include <QSize>
#include <QString>

class ProjectSettings
{
public:
    enum class CacheQuality {
        Quarter16,
        Quarter8,
        Quarter4,
        Half,
        Source
    };

    int fps = 30;
    QSize resolution = QSize(1280, 720);
    CacheQuality cacheQuality = CacheQuality::Quarter4;
    // 0 = auto (1 frame at current project FPS), otherwise explicit lookback in ms.
    int decodeSeekLookbackMs = 0;

    double cacheScaleFactor() const;
    QString cacheQualityLabel() const;
    QSize cacheResolution() const;
    int effectiveDecodeSeekLookbackMs() const;
};

#endif // PROJECTSETTINGS_H
