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

    double cacheScaleFactor() const;
    QString cacheQualityLabel() const;
    QSize cacheResolution() const;
};

#endif // PROJECTSETTINGS_H
