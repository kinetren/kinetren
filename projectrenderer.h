#ifndef PROJECTRENDERER_H
#define PROJECTRENDERER_H

#include <QString>
#include <atomic>
#include <functional>
#include <vector>

#include "projectsettings.h"
#include "timelineeditor.h"

class ProjectRenderer
{
public:
    struct Result {
        bool ok = false;
        bool cancelled = false;
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
        double processedOutputSeconds = 0;
        double speedRealtimeFactor = 0;
    };

    using ProgressCallback = std::function<void(const RenderProgress &)>;
    using LogCallback = std::function<void(const QString &)>;

    static Result renderMp4(const std::vector<TimelineEditor::RenderClip> &clips,
                            const ProjectSettings &settings,
                            qint64 startMs,
                            qint64 endMs,
                            const QString &outPath,
                            const ProgressCallback &onProgress = {},
                            std::atomic<bool> *cancelFlag = nullptr,
                            std::atomic<bool> *pauseFlag = nullptr,
                            const LogCallback &logLine = {});
};

#endif // PROJECTRENDERER_H
