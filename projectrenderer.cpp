#include "projectrenderer.h"

#include <QDir>
#include <QFile>
#include <QProcess>
#include <QTemporaryDir>
#include <QTemporaryFile>

#include <algorithm>

namespace {
QString secString(qint64 ms)
{
    return QString::number(static_cast<double>(std::max<qint64>(0, ms)) / 1000.0, 'f', 6);
}
}

ProjectRenderer::Result ProjectRenderer::renderMp4(const std::vector<TimelineEditor::RenderClip> &clips,
                                                   const ProjectSettings &settings,
                                                   qint64 startMs,
                                                   qint64 endMs,
                                                   const QString &outPath)
{
    Result result;
    if (endMs <= startMs) {
        result.message = "Empty render range.";
        return result;
    }
    if (outPath.isEmpty()) {
        result.message = "Output path is empty.";
        return result;
    }

    struct InputClip {
        TimelineEditor::RenderClip clip;
        int inputIndex = -1;
    };

    std::vector<InputClip> videoInputs;
    std::vector<InputClip> audioInputs;
    videoInputs.reserve(clips.size());
    audioInputs.reserve(clips.size());

    int nextInput = 0;
    for (const auto &clip : clips) {
        const qint64 clipEnd = clip.startMs + clip.durationMs;
        if (clipEnd <= startMs || clip.startMs >= endMs) {
            continue;
        }
        if (clip.isVideo) {
            videoInputs.push_back({clip, nextInput++});
        }
        if (clip.isAudio) {
            audioInputs.push_back({clip, nextInput++});
        }
    }

    QStringList args;
    args << "-y";
    for (const auto &v : videoInputs) {
        args << "-ss" << secString(v.clip.sourceInMs);
        args << "-t" << secString(v.clip.durationMs);
        args << "-i" << v.clip.sourcePath;
    }
    for (const auto &a : audioInputs) {
        args << "-ss" << secString(a.clip.sourceInMs);
        args << "-t" << secString(a.clip.durationMs);
        args << "-i" << a.clip.sourcePath;
    }

    QStringList filterLines;
    const qint64 totalMs = endMs - startMs;
    filterLines << QString("color=c=black:s=%1x%2:r=%3:d=%4[base0]")
                       .arg(settings.resolution.width())
                       .arg(settings.resolution.height())
                       .arg(settings.fps)
                       .arg(secString(totalMs));

    std::sort(videoInputs.begin(), videoInputs.end(), [](const InputClip &lhs, const InputClip &rhs) {
        if (lhs.clip.trackIndex != rhs.clip.trackIndex) {
            return lhs.clip.trackIndex < rhs.clip.trackIndex;
        }
        return lhs.clip.startMs < rhs.clip.startMs;
    });

    int videoNodeIdx = 0;
    for (const auto &v : videoInputs) {
        const qint64 delayMs = std::max<qint64>(0, v.clip.startMs - startMs);
        const QString inLabel = QString("[%1:v]").arg(v.inputIndex);
        const QString scaled = QString("[v_scaled_%1]").arg(videoNodeIdx);
        const QString delayed = QString("[v_delay_%1]").arg(videoNodeIdx);
        filterLines << QString("%1setpts=PTS-STARTPTS,scale=%2:%3:force_original_aspect_ratio=decrease,pad=%2:%3:(ow-iw)/2:(oh-ih)/2,format=yuv420p%4")
                           .arg(inLabel)
                           .arg(settings.resolution.width())
                           .arg(settings.resolution.height())
                           .arg(scaled);
        filterLines << QString("%1setpts=PTS+%2/TB%3")
                           .arg(scaled)
                           .arg(delayMs)
                           .arg(delayed);
        const QString under = (videoNodeIdx == 0) ? "[base0]" : QString("[v_mix_%1]").arg(videoNodeIdx - 1);
        const QString out = QString("[v_mix_%1]").arg(videoNodeIdx);
        filterLines << QString("%1%2overlay=eof_action=pass:shortest=0%3")
                           .arg(under)
                           .arg(delayed)
                           .arg(out);
        ++videoNodeIdx;
    }

    int audioNodeIdx = 0;
    for (const auto &a : audioInputs) {
        const qint64 delayMs = std::max<qint64>(0, a.clip.startMs - startMs);
        const QString inLabel = QString("[%1:a]").arg(a.inputIndex);
        const QString out = QString("[a_%1]").arg(audioNodeIdx);
        filterLines << QString("%1atrim=0:%2,asetpts=PTS-STARTPTS,adelay=%3|%3%4")
                           .arg(inLabel)
                           .arg(secString(a.clip.durationMs))
                           .arg(delayMs)
                           .arg(out);
        ++audioNodeIdx;
    }

    const QString vOutLabel = (videoNodeIdx > 0) ? QString("[v_mix_%1]").arg(videoNodeIdx - 1) : QString("[base0]");
    QString aOutLabel;
    if (audioNodeIdx > 0) {
        QString mixIns;
        for (int i = 0; i < audioNodeIdx; ++i) {
            mixIns += QString("[a_%1]").arg(i);
        }
        aOutLabel = "[a_out]";
        filterLines << QString("%1amix=inputs=%2:normalize=0:dropout_transition=0%3")
                           .arg(mixIns)
                           .arg(audioNodeIdx)
                           .arg(aOutLabel);
    } else {
        aOutLabel = "[a_silent]";
        filterLines << QString("anullsrc=r=48000:cl=stereo,atrim=0:%1%2")
                           .arg(secString(totalMs))
                           .arg(aOutLabel);
    }

    QTemporaryDir tmpDir;
    if (!tmpDir.isValid()) {
        result.message = "Cannot create temporary directory for renderer.";
        return result;
    }
    const QString filterPath = tmpDir.path() + "/ffmpeg-filter.txt";
    QFile filterFile(filterPath);
    if (!filterFile.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        result.message = "Cannot create ffmpeg filter script.";
        return result;
    }
    filterFile.write(filterLines.join(";\n").toUtf8());
    filterFile.close();

    args << "-filter_complex_script" << filterPath;
    args << "-map" << vOutLabel;
    args << "-map" << aOutLabel;
    args << "-r" << QString::number(settings.fps);
    args << "-c:v" << "libx264";
    args << "-pix_fmt" << "yuv420p";
    args << "-preset" << "veryfast";
    args << "-crf" << "23";
    args << "-c:a" << "aac";
    args << "-b:a" << "192k";
    args << "-movflags" << "+faststart";
    args << outPath;

    QProcess ffmpeg;
    ffmpeg.start("ffmpeg", args);
    if (!ffmpeg.waitForStarted(4000)) {
        result.message = "ffmpeg is not available in PATH.";
        return result;
    }
    ffmpeg.waitForFinished(-1);
    if (ffmpeg.exitStatus() != QProcess::NormalExit || ffmpeg.exitCode() != 0) {
        result.message = QString("ffmpeg failed: %1").arg(QString::fromUtf8(ffmpeg.readAllStandardError()));
        return result;
    }

    result.ok = true;
    result.message = QString("Render complete: %1").arg(outPath);
    return result;
}
