#ifndef BC_FX_H
#define BC_FX_H

#include "bc.h"

#include <QJsonArray>
#include <QJsonObject>
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
               BurstQuality projectOutput);

    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;
    void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) override;

private:
    std::shared_ptr<IVideoFx> effect;
    std::shared_ptr<IBurstcon> upstream;
    BurstQuality projectOutput;
};

class ConAudioFx : public IBurstcon
{
public:
    ConAudioFx(std::shared_ptr<IAudioFx> effect, std::shared_ptr<IBurstcon> upstream);

    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;
    void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) override;

private:
    std::shared_ptr<IAudioFx> effect;
    std::shared_ptr<IBurstcon> upstream;
};

std::shared_ptr<IVideoFx> createVideoFx(const QString &typeId, const QJsonObject &params);
std::shared_ptr<IAudioFx> createAudioFx(const QString &typeId, const QJsonObject &params);

/// Each JSON element: `{ "type": "<id>", "params": { ... } }` — first entry sits closest to the decoder.
std::shared_ptr<IBurstcon> wrapVideoFxChain(std::shared_ptr<IBurstcon> media,
                                            const QJsonArray &chain,
                                            const BurstQuality &projectOutput);
std::shared_ptr<IBurstcon> wrapAudioFxChain(std::shared_ptr<IBurstcon> media, const QJsonArray &chain);

/// Single `pan` effect with project-default framing (used when a clip has no saved chain).
QJsonArray defaultVideoFxChain();

/// Applies the same video chain as playback (for UI preview). Mutates `vb.frame` / `vb.quality`.
bool processVideoFxChainInPlace(VideoBurst &vb, const QJsonArray &chain, const VideoFxContext &ctx);

} // namespace burstcon

#endif
