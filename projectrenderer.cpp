#include "projectrenderer.h"

#include <QElapsedTimer>
#include <QFile>
#include <QProcess>
#include <QSaveFile>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QThread>
#include <QDir>
#include <QDateTime>

#include <algorithm>
#include <climits>
#include <cmath>

#include "burstcon.h"

namespace {
constexpr int kAudioSampleRate = 48000;
constexpr int kAudioChannels = 2;
constexpr int kAudioSamplesPerBurst = 1024;

qint64 msToSampleFrames(qint64 ms)
{
    return (std::max<qint64>(0, ms) * kAudioSampleRate) / 1000;
}

qint64 ticksToSamplesRounded(burstcon::BurstIndex ticks)
{
    const burstcon::BurstIndex clamped = std::max<burstcon::BurstIndex>(0, ticks);
    return static_cast<qint64>((clamped * kAudioSampleRate + (burstcon::kBurstTimebaseHz / 2)) / burstcon::kBurstTimebaseHz);
}

burstcon::BurstIndex msToBurstIndexCeil(qint64 ms)
{
    const qint64 safe = std::max<qint64>(1, ms);
    return static_cast<burstcon::BurstIndex>((safe * burstcon::kBurstTimebaseHz + 999) / 1000);
}

QSize evenSize(const QSize &in)
{
    int w = std::max(2, in.width());
    int h = std::max(2, in.height());
    if ((w % 2) != 0) {
        ++w;
    }
    if ((h % 2) != 0) {
        ++h;
    }
    return QSize(w, h);
}
}

ProjectRenderer::Result ProjectRenderer::renderMp4(const std::vector<TimelineEditor::RenderClip> &clips,
                                                   const ProjectSettings &settings,
                                                   qint64 startMs,
                                                   qint64 endMs,
                                                   const QString &outPath,
                                                   const ProgressCallback &onProgress,
                                                   std::atomic<bool> *cancelFlag,
                                                   std::atomic<bool> *pauseFlag,
                                                   const LogCallback &logLine)
{
    Result result;
    auto log = [&](const QString &s) {
        if (logLine) {
            logLine(s);
        }
    };
    auto cancelled = [&]() -> bool { return cancelFlag && cancelFlag->load(); };
    auto waitWhilePaused = [&]() -> bool {
        while (pauseFlag && pauseFlag->load()) {
            if (cancelled()) {
                return false;
            }
            QThread::msleep(40);
        }
        return !cancelled();
    };
    if (endMs <= startMs) {
        result.message = "Empty render range.";
        return result;
    }
    if (outPath.isEmpty()) {
        result.message = "Output path is empty.";
        return result;
    }
    if (settings.fps <= 0) {
        result.message = "Invalid FPS in project settings.";
        return result;
    }
    const QSize renderSize = evenSize(settings.cacheResolution());
    if (renderSize.width() <= 0 || renderSize.height() <= 0) {
        result.message = "Invalid render resolution in project settings.";
        return result;
    }

    std::vector<burstcon::ConTrackEvent> videoEvents;
    std::vector<burstcon::ConTrackEvent> audioEvents;
    videoEvents.reserve(clips.size());
    audioEvents.reserve(clips.size());
    int audioZ = 0;
    for (const auto &clip : clips) {
        const qint64 clipEnd = clip.startMs + clip.durationMs;
        if (clipEnd <= startMs || clip.startMs >= endMs) {
            continue;
        }
        const qint64 effectiveStartMs = std::max(startMs, clip.startMs);
        const qint64 effectiveEndMs = std::min(endMs, clipEnd);
        const qint64 effectiveDurationMs = std::max<qint64>(0, effectiveEndMs - effectiveStartMs);
        if (effectiveDurationMs <= 0) {
            continue;
        }
        if (clip.isVideo) {
            burstcon::ConTrackEvent e;
            e.sourcePath = clip.sourcePath;
            e.start = burstcon::msToBurstIndex(effectiveStartMs);
            e.duration = msToBurstIndexCeil(effectiveDurationMs);
            e.sourceIn = burstcon::msToBurstIndex(clip.sourceInMs + (effectiveStartMs - clip.startMs));
            e.hasVideo = true;
            e.hasAudio = false;
            const qint64 startKey = std::min<qint64>(std::max<qint64>(0, effectiveStartMs), 999'999LL);
            e.zIndex = -static_cast<int>(clip.trackIndex * 1'000'000LL + startKey);
            videoEvents.push_back(e);
        }
        if (clip.isAudio) {
            burstcon::ConTrackEvent e;
            e.sourcePath = clip.sourcePath;
            e.start = burstcon::msToBurstIndex(effectiveStartMs);
            e.duration = msToBurstIndexCeil(effectiveDurationMs);
            e.sourceIn = burstcon::msToBurstIndex(clip.sourceInMs + (effectiveStartMs - clip.startMs));
            e.hasVideo = false;
            e.hasAudio = true;
            e.zIndex = audioZ++;
            audioEvents.push_back(e);
        }
    }

    burstcon::ConMediaPoolConfig poolCfg;
    poolCfg.enableConMediaCache = true;
    poolCfg.cacheConfig.maxEntries = 128;
    poolCfg.cacheConfig.maxAudioEntries = 64;
    poolCfg.cacheConfig.maxVideoEntries = 64;
    poolCfg.cacheConfig.maxVideoBytes = 128ULL * 1024ULL * 1024ULL;
    poolCfg.cacheConfig.preloadEnabled = false;
    poolCfg.maxMediaInstances = 48;
    auto mediaPool = std::make_shared<burstcon::ConMediaPool>(poolCfg);
    auto videoTrack = std::make_shared<burstcon::ConTrack>(mediaPool);
    auto audioTrack = std::make_shared<burstcon::ConTrack>(mediaPool);
    videoTrack->setEvents(videoEvents);
    audioTrack->setEvents(audioEvents);
    auto trimmer = std::make_shared<burstcon::ConTrimmer>();
    trimmer->setTracks({videoTrack, audioTrack});
    log(QStringLiteral("Events: video=%1 audio=%2").arg(videoEvents.size()).arg(audioEvents.size()));

    QTemporaryDir tmpDir;
    if (!tmpDir.isValid()) {
        result.message = "Cannot create temporary directory for renderer.";
        return result;
    }
    const QString rawVideoPath = tmpDir.path() + "/video.rgb24";
    const QString rawAudioPath = tmpDir.path() + "/audio.s16le";
    QFile rawVideo(rawVideoPath);
    QFile rawAudio(rawAudioPath);
    if (!rawVideo.open(QIODevice::WriteOnly | QIODevice::Truncate) ||
        !rawAudio.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        result.message = "Cannot create temporary raw streams for renderer.";
        return result;
    }

    const qint64 totalMs = endMs - startMs;
    const int frameCount = std::max(
        1,
        static_cast<int>(std::ceil(static_cast<double>(totalMs) * static_cast<double>(settings.fps) / 1000.0)));

    QElapsedTimer wallClock;
    wallClock.start();
    auto report = [&](RenderProgress p) {
        p.elapsedMs = wallClock.elapsed();
        p.outputDurationMs = totalMs;
        const double elapsedSec = std::max(1e-6, static_cast<double>(p.elapsedMs) / 1000.0);
        p.speedRealtimeFactor = p.processedOutputSeconds / elapsedSec;
        if (onProgress) {
            onProgress(p);
        }
    };

    {
        RenderProgress p;
        p.phase = RenderProgress::Phase::VideoFrames;
        p.done = 0;
        p.total = frameCount;
        p.outputDurationMs = totalMs;
        p.processedOutputSeconds = 0;
        report(p);
    }
    log(QStringLiteral("Video: %1×%2 @ %3 fps, %4 frames (~%5 s output)")
            .arg(renderSize.width())
            .arg(renderSize.height())
            .arg(settings.fps)
            .arg(frameCount)
            .arg(static_cast<double>(totalMs) / 1000.0, 0, 'f', 2));

    std::vector<char> blackFrame(static_cast<size_t>(renderSize.width()) * static_cast<size_t>(renderSize.height()) * 3, 0);
    for (int frameIdx = 0; frameIdx < frameCount; ++frameIdx) {
        if (!waitWhilePaused()) {
            result.cancelled = true;
            result.message = QStringLiteral("Cancelled.");
            log(result.message);
            return result;
        }
        if (cancelled()) {
            result.cancelled = true;
            result.message = QStringLiteral("Cancelled.");
            log(result.message);
            return result;
        }
        const double frameMsF = static_cast<double>(startMs) + (1000.0 * static_cast<double>(frameIdx) / static_cast<double>(settings.fps));
        const qint64 frameMs = static_cast<qint64>(frameMsF);
        burstcon::BurstRequest req;
        const int outFps = std::max(1, settings.fps);
        req.index = burstcon::alignBurstIndexToOutputVideoFrame(outFps, burstcon::msToBurstIndex(frameMs));
        req.quality = burstcon::BurstQuality{renderSize.width(), renderSize.height()};
        burstcon::VideoBurst vb;
        if (!trimmer->videoBurst(req, vb) || vb.frame.isNull()) {
            if (rawVideo.write(blackFrame.data(), static_cast<qint64>(blackFrame.size())) != static_cast<qint64>(blackFrame.size())) {
                result.message = "Failed while writing temporary video stream.";
                return result;
            }
            continue;
        }
        QImage rgb = vb.frame.convertToFormat(QImage::Format_RGB888);
        if (rgb.size() != renderSize) {
            rgb = rgb.scaled(renderSize, Qt::IgnoreAspectRatio, Qt::FastTransformation);
        }
        const qint64 bytesPerFrame = static_cast<qint64>(renderSize.width()) * static_cast<qint64>(renderSize.height()) * 3;
        if (rgb.sizeInBytes() < bytesPerFrame) {
            result.message = "Preview video frame has unexpected format/size.";
            return result;
        }
        if (rawVideo.write(reinterpret_cast<const char *>(rgb.constBits()), bytesPerFrame) != bytesPerFrame) {
            result.message = "Failed while writing temporary video stream.";
            return result;
        }
        if (onProgress && (frameIdx % 8 == 0 || frameIdx + 1 == frameCount)) {
            RenderProgress p;
            p.phase = RenderProgress::Phase::VideoFrames;
            p.done = frameIdx + 1;
            p.total = frameCount;
            p.outputDurationMs = totalMs;
            p.processedOutputSeconds = static_cast<double>(frameIdx + 1) / static_cast<double>(settings.fps);
            report(p);
        }
    }
    log(QStringLiteral("Raw video stream closed."));

    const burstcon::BurstIndex burstStep =
        std::max<burstcon::BurstIndex>(1, (burstcon::kBurstTimebaseHz * kAudioSamplesPerBurst) / kAudioSampleRate);
    const burstcon::BurstIndex startIndex = burstcon::msToBurstIndex(startMs);
    const burstcon::BurstIndex endIndex = burstcon::msToBurstIndex(endMs);
    const burstcon::BurstIndex firstIndex = (startIndex / burstStep) * burstStep;
    const qint64 totalSampleFrames = msToSampleFrames(totalMs);
    qint64 writtenSampleFrames = 0;

    qint64 audioBurstTotal = 0;
    for (burstcon::BurstIndex scan = firstIndex; scan < endIndex; scan += burstStep) {
        ++audioBurstTotal;
    }
    audioBurstTotal = std::max<qint64>(1, audioBurstTotal);
    const int audioTotalInt = static_cast<int>(std::min<qint64>(audioBurstTotal, INT_MAX));
    {
        RenderProgress p;
        p.phase = RenderProgress::Phase::AudioPcm;
        p.done = 0;
        p.total = audioTotalInt;
        p.outputDurationMs = totalMs;
        p.processedOutputSeconds = (totalMs / 1000.0) * 0.0;
        report(p);
    }
    log(QStringLiteral("Writing PCM: %1 sample frames (~%2 s @ %3 Hz).")
            .arg(totalSampleFrames)
            .arg(static_cast<double>(totalSampleFrames) / static_cast<double>(kAudioSampleRate), 0, 'f', 2)
            .arg(kAudioSampleRate));

    qint64 burstIter = 0;
    for (burstcon::BurstIndex idx = firstIndex; idx < endIndex && writtenSampleFrames < totalSampleFrames; idx += burstStep) {
        if (!waitWhilePaused()) {
            result.cancelled = true;
            result.message = QStringLiteral("Cancelled.");
            log(result.message);
            return result;
        }
        if (cancelled()) {
            result.cancelled = true;
            result.message = QStringLiteral("Cancelled.");
            log(result.message);
            return result;
        }
        ++burstIter;
        const burstcon::BurstIndex burstStart = idx;
        const burstcon::BurstIndex burstEnd = idx + burstStep;
        const burstcon::BurstIndex overlapStart = std::max(burstStart, startIndex);
        const burstcon::BurstIndex overlapEnd = std::min(burstEnd, endIndex);
        if (overlapEnd <= overlapStart) {
            continue;
        }

        burstcon::BurstRequest req;
        req.index = overlapStart;
        burstcon::AudioBurst ab;
        if (!trimmer->audioBurst(req, ab)) {
            ab.index = idx;
            ab.sampleRate = kAudioSampleRate;
            ab.channels = kAudioChannels;
            ab.samples.assign(static_cast<size_t>(kAudioSamplesPerBurst * kAudioChannels), 0);
        }
        const int channels = std::max(1, ab.channels);
        const qint64 fullFrames = static_cast<qint64>(ab.samples.size()) / channels;
        if (fullFrames <= 0) {
            continue;
        }

        qint64 startFrameInBurst = ticksToSamplesRounded(overlapStart - burstStart);
        qint64 keepFrames = ticksToSamplesRounded(overlapEnd - overlapStart);
        startFrameInBurst = std::clamp<qint64>(startFrameInBurst, 0, fullFrames);
        keepFrames = std::clamp<qint64>(keepFrames, 0, fullFrames - startFrameInBurst);
        keepFrames = std::min<qint64>(keepFrames, totalSampleFrames - writtenSampleFrames);
        if (keepFrames <= 0) {
            continue;
        }

        const int64_t startSample = startFrameInBurst * channels;
        const int64_t sampleCount = keepFrames * channels;
        std::vector<int16_t> pcm(static_cast<size_t>(sampleCount), 0);
        for (int64_t i = 0; i < sampleCount; ++i) {
            const int32_t s = ab.samples[static_cast<size_t>(startSample + i)];
            pcm[static_cast<size_t>(i)] = static_cast<int16_t>(std::clamp<int32_t>(s, -32768, 32767));
        }
        const qint64 bytesToWrite = static_cast<qint64>(pcm.size() * sizeof(int16_t));
        if (rawAudio.write(reinterpret_cast<const char *>(pcm.data()), bytesToWrite) != bytesToWrite) {
            result.message = "Failed while writing temporary audio stream.";
            return result;
        }
        writtenSampleFrames += keepFrames;
        if (onProgress && (burstIter % 32 == 0 || burstIter == audioBurstTotal || writtenSampleFrames >= totalSampleFrames)) {
            const int adone = static_cast<int>(std::min<qint64>(burstIter, audioBurstTotal));
            RenderProgress p;
            p.phase = RenderProgress::Phase::AudioPcm;
            p.done = adone;
            p.total = audioTotalInt;
            p.outputDurationMs = totalMs;
            p.processedOutputSeconds =
                (totalMs / 1000.0) * (static_cast<double>(writtenSampleFrames) / static_cast<double>(std::max<qint64>(1, totalSampleFrames)));
            report(p);
        }
    }

    if (onProgress) {
        RenderProgress p;
        p.phase = RenderProgress::Phase::AudioPcm;
        p.done = audioTotalInt;
        p.total = audioTotalInt;
        p.outputDurationMs = totalMs;
        p.processedOutputSeconds = totalMs / 1000.0;
        report(p);
    }

    if (writtenSampleFrames < totalSampleFrames) {
        const qint64 missingFrames = totalSampleFrames - writtenSampleFrames;
        std::vector<int16_t> silence(static_cast<size_t>(missingFrames * kAudioChannels), 0);
        const qint64 bytesToWrite = static_cast<qint64>(silence.size() * sizeof(int16_t));
        if (rawAudio.write(reinterpret_cast<const char *>(silence.data()), bytesToWrite) != bytesToWrite) {
            result.message = "Failed while writing trailing silence.";
            return result;
        }
    }

    rawVideo.close();
    rawAudio.close();
    log(QStringLiteral("Starting ffmpeg mux (libx264 + AAC)."));

    {
        RenderProgress p;
        p.phase = RenderProgress::Phase::Muxing;
        p.done = 0;
        p.total = 1;
        p.outputDurationMs = totalMs;
        p.processedOutputSeconds = totalMs / 1000.0 * 0.98;
        report(p);
    }

    QStringList args;
    args << "-y";
    args << "-f" << "rawvideo";
    args << "-pix_fmt" << "rgb24";
    args << "-s" << QString("%1x%2").arg(renderSize.width()).arg(renderSize.height());
    args << "-r" << QString::number(settings.fps);
    args << "-i" << rawVideoPath;
    args << "-f" << "s16le";
    args << "-ar" << QString::number(kAudioSampleRate);
    args << "-ac" << QString::number(kAudioChannels);
    args << "-i" << rawAudioPath;
    args << "-c:v" << "libx264";
    args << "-pix_fmt" << "yuv420p";
    args << "-preset" << "veryfast";
    args << "-crf" << "23";
    args << "-c:a" << "aac";
    args << "-b:a" << "192k";
    args << "-movflags" << "+faststart";
    args << "-shortest";
    args << outPath;

    log(QStringLiteral("ffmpeg %1").arg(args.join(QLatin1Char(' '))));

    QProcess ffmpeg;
    ffmpeg.start("ffmpeg", args);
    if (!ffmpeg.waitForStarted(4000)) {
        result.message = "ffmpeg is not available in PATH.";
        result.details = "Process failed to start within 4 seconds.";
        log(result.message + QLatin1Char(' ') + result.details);
        return result;
    }
    while (!ffmpeg.waitForFinished(200)) {
        if (!waitWhilePaused()) {
            ffmpeg.kill();
            ffmpeg.waitForFinished(8000);
            result.cancelled = true;
            result.message = QStringLiteral("Cancelled.");
            log(QStringLiteral("ffmpeg killed (pause/cancel)."));
            return result;
        }
        if (cancelled()) {
            ffmpeg.kill();
            ffmpeg.waitForFinished(8000);
            result.cancelled = true;
            result.message = QStringLiteral("Cancelled.");
            log(QStringLiteral("ffmpeg killed."));
            return result;
        }
        const QByteArray err = ffmpeg.readAllStandardError();
        if (!err.isEmpty()) {
            const QString chunk = QString::fromUtf8(err).trimmed();
            if (!chunk.isEmpty()) {
                log(chunk);
            }
        }
    }
    {
        const QByteArray errTail = ffmpeg.readAllStandardError();
        if (!errTail.isEmpty()) {
            log(QString::fromUtf8(errTail).trimmed());
        }
    }
    if (ffmpeg.exitStatus() != QProcess::NormalExit || ffmpeg.exitCode() != 0) {
        const QString stderrText = QString::fromUtf8(ffmpeg.readAllStandardError());
        const QString stdoutText = QString::fromUtf8(ffmpeg.readAllStandardOutput());
        result.message = QString("ffmpeg failed (exit code %1)").arg(ffmpeg.exitCode());
        result.details = QString("Command: ffmpeg %1\n\nSTDERR:\n%2\n\nSTDOUT:\n%3")
                             .arg(args.join(' '))
                             .arg(stderrText)
                             .arg(stdoutText);

        QString logDir = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
        if (logDir.isEmpty()) {
            logDir = QDir::homePath() + "/.kp-test";
        }
        QDir dir(logDir);
        if (!dir.exists()) {
            dir.mkpath(".");
        }
        const QString logPath = dir.filePath(
            "render-error-" + QDateTime::currentDateTimeUtc().toString("yyyyMMdd-hhmmss") + ".log");
        QSaveFile logFile(logPath);
        if (logFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            logFile.write(result.details.toUtf8());
            if (logFile.commit()) {
                result.logPath = logPath;
            }
        }
        log(result.message);
        return result;
    }

    {
        RenderProgress p;
        p.phase = RenderProgress::Phase::Muxing;
        p.done = 1;
        p.total = 1;
        p.outputDurationMs = totalMs;
        p.processedOutputSeconds = totalMs / 1000.0;
        report(p);
    }

    result.ok = true;
    result.message = QString("Render complete: %1").arg(outPath);
    log(result.message);
    return result;
}
