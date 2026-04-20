#ifndef PROJECTRENDERER_H
#define PROJECTRENDERER_H

#include <QString>
#include <functional>
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

    struct RenderProgress {
        enum class Phase {
            VideoFrames,
            AudioPcm,
            Muxing
        };
        Phase phase = Phase::VideoFrames;
        int done = 0;
        int total = 1;
        qint64 elapsedMs = 0;
        qint64 outputDurationMs = 0;
    };

    using ProgressCallback = std::function<void(const RenderProgress &)>;

    static Result renderMp4(const std::vector<TimelineEditor::RenderClip> &clips,
                            const ProjectSettings &settings,
                            qint64 startMs,
                            qint64 endMs,
                            const QString &outPath,
                            const ProgressCallback &onProgress = {});
};

#endif // PROJECTRENDERER_H
