#ifndef PROJECTRENDERER_H
#define PROJECTRENDERER_H

#include <QString>
#include <vector>

#include "projectsettings.h"
#include "timelineeditor.h"

class ProjectRenderer
{
public:
    struct Result {
        bool ok = false;
        QString message;
        QString details;
        QString logPath;
    };

    static Result renderMp4(const std::vector<TimelineEditor::RenderClip> &clips,
                            const ProjectSettings &settings,
                            qint64 startMs,
                            qint64 endMs,
                            const QString &outPath);
};

#endif // PROJECTRENDERER_H
