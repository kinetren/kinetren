#ifndef BC_FX_H
#define BC_FX_H

#include "bc.h"

#include <QImage>
#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QString>

#include <memory>

namespace burstcon {

struct VideoFxContext {
    BurstQuality projectOutput;
    /// Media decode index passed with the burst request (same as `BurstRequest::index` under `ConVideoFx`).
    BurstIndex timelineBurstIndex = 0;
    BurstOriginInfo mediaOrigin;
    /// From `BurstRequest::clipDurationTicks` when the track supplies clip bounds.
    BurstIndex clipDurationTicks = 0;
    /// From `BurstRequest::decodeOrigin` (clip `sourceIn` in media timebase).
    BurstIndex decodeOrigin = -1;
};

/// Video effect (Vegas-style chain element). Stateless apart from JSON params.
class IVideoFx
{
public:
    virtual ~IVideoFx() = default;
    virtual QString typeId() const = 0;
    virtual void setParams(const QJsonObject &o) = 0;
    virtual QJsonObject params() const = 0;
    virtual bool process(VideoBurst &inOut, const VideoFxContext &ctx) = 0;
};

struct AudioFxContext {
    BurstIndex timelineBurstIndex = 0;
    int sampleRate = 48000;
    int channels = 2;
    /// Same semantics as `BurstRequest::clipDurationTicks` / `decodeOrigin` for automation along the clip.
    BurstIndex clipDurationTicks = 0;
    BurstIndex decodeOrigin = -1;
};

class IAudioFx
{
public:
    virtual ~IAudioFx() = default;
    virtual QString typeId() const = 0;
    virtual void setParams(const QJsonObject &o) = 0;
    virtual QJsonObject params() const = 0;
    virtual bool process(AudioBurst &inOut, const AudioFxContext &ctx) = 0;
};

/// Wraps upstream `IBurstcon` (usually ConCache→ConMedia) and runs one video effect after decode.
class ConVideoFx : public IBurstcon
{
public:
    ConVideoFx(std::shared_ptr<IVideoFx> effect,
               std::shared_ptr<IBurstcon> upstream,
               BurstQuality projectOutput,
               QJsonObject storedParams = {});

    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;
    void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) override;

private:
    std::shared_ptr<IVideoFx> effect;
    std::shared_ptr<IBurstcon> upstream;
    BurstQuality projectOutput;
    QJsonObject storedParams;
};

class ConAudioFx : public IBurstcon
{
public:
    ConAudioFx(std::shared_ptr<IAudioFx> effect,
               std::shared_ptr<IBurstcon> upstream,
               QJsonObject storedParams = {});

    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;
    void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) override;

private:
    std::shared_ptr<IAudioFx> effect;
    std::shared_ptr<IBurstcon> upstream;
    QJsonObject storedParams;
    BurstIndex lastWarpTimelineIndex = -1;
    std::vector<float> warpOverlapTail;
};

std::shared_ptr<IVideoFx> createVideoFx(const QString &typeId, const QJsonObject &params);
std::shared_ptr<IAudioFx> createAudioFx(const QString &typeId, const QJsonObject &params);

/// Timeline→source speed from audio FX (1.0 = normal). &lt;1 stretches, &gt;1 compresses timeline vs source.
double audioTimelineSourceRate(const QJsonArray &chain);
/// When true, UI must not auto-resize the clip for playback-rate / stretch FX.
bool audioFxLockTimelineDuration(const QJsonArray &chain);
/// Source offset in media ticks for `timelineRel` within `timelineDur` (handles playback-rate automation).
BurstIndex audioTimelineToSourceOffsetTicks(const QJsonArray &chain,
                                            BurstIndex timelineRel,
                                            BurstIndex timelineDur);

/// Each JSON element: `{ "type": "<id>", "params": { ... } }` — first entry sits closest to the decoder.
std::shared_ptr<IBurstcon> wrapVideoFxChain(std::shared_ptr<IBurstcon> media,
                                            const QJsonArray &chain,
                                            const BurstQuality &projectOutput);
std::shared_ptr<IBurstcon> wrapAudioFxChain(std::shared_ptr<IBurstcon> media, const QJsonArray &chain);

/// Single `pan` effect with project-default framing (used when a clip has no saved chain).
QJsonArray defaultVideoFxChain();

/// Applies the same video chain as playback (for UI preview). Mutates `vb.frame` / `vb.quality`.
bool processVideoFxChainInPlace(VideoBurst &vb, const QJsonArray &chain, const VideoFxContext &ctx);

/// Applies the same audio chain as playback (for UI preview). Mutates `ab.samples`.
bool processAudioFxChainInPlace(AudioBurst &ab, const QJsonArray &chain, const AudioFxContext &ctx);

/// `frames[0]` is the reference texture at clip start; all frames same size (e.g. 320×180 decode).
/// Returns a `keyframes` JSON array: `{ "t", "panX", "panY", "zoom" }` in [0,1] time along the clip.
/// `targetKeyframeCount` > 0: RDP simplification is tuned to leave about that many keys (else ~n/3, min 6).
QJsonArray estimateCamMotionFromDecodedFrames(const QList<QImage> &frames,
                                              int outW,
                                              int outH,
                                              const QString &mode = QStringLiteral("fit"),
                                              int targetKeyframeCount = 0);

} // namespace burstcon

#endif
