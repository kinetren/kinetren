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
    /// FrontScaler (preview panel): when true, decode RGB near cacheResolution() and scale to the
    /// preview widget in Qt; when false, full-resolution decode (BurstQuality 0×0) then Qt scales.
    /// libswscale still performs YUV→RGB at the chosen decode size.
    bool scaleVideoAtDisplayTime = true;
    bool previewSmoothScaling = false;
    bool previewAntialiasing = false;
    // 0 = auto (1 frame at current project FPS), otherwise explicit lookback in ms.
    int decodeSeekLookbackMs = 0;

    double cacheScaleFactor() const;
    QString cacheQualityLabel() const;
    QSize cacheResolution() const;
    int effectiveDecodeSeekLookbackMs() const;
};

#endif // PROJECTSETTINGS_H
