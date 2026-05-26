#ifndef BURSTCON_H
#define BURSTCON_H

#include <QImage>
#include <QJsonArray>
#include <QString>

#include <atomic>
#include <cstdint>
#include <functional>
#include <list>
#include <limits>
#include <memory>
#include <mutex>
#include <condition_variable>
#include <string>
#include <thread>
#include <unordered_map>
#include <variant>
#include <vector>

class QPainter;

namespace burstcon {

constexpr int64_t kBurstTimebaseHz = 90000;

using BurstIndex = int64_t;

struct BurstQuality {
    int width = 0;
    int height = 0;
};

struct BurstRange {
    BurstIndex start = 0;
    BurstIndex end = 0;
};

struct BurstRequest {
    BurstIndex index = 0;
    BurstQuality quality;
    BurstIndex decodeOrigin = -1;
    /// Trimmed clip length in burst ticks (same as `ConTrackEvent::duration`). Video FX use this with
    /// `decodeOrigin` for normalized time along the clip. 0 means unknown (e.g. preview / tests).
    BurstIndex clipDurationTicks = 0;
    /// Global timeline index for audio FX automation / warp (ConTrack). -1 = use `index`.
    BurstIndex timelineBurstIndex = -1;
    /// Media `sourceIn` for the clip (audio warp / decode). -1 = use `decodeOrigin`.
    BurstIndex clipSourceIn = -1;
};

enum class BurstType {
    Audio,
    Video
};

struct AudioBurst {
    BurstIndex index = 0;
    int sampleRate = 48000;
    int channels = 2;
    std::vector<int32_t> samples;
};

struct VideoBurst {
    BurstIndex index = 0;
    BurstQuality quality;
    QImage frame;
};

struct BurstOriginInfo {
    BurstQuality sourceVideoQuality;
    int sourceAudioSampleRate = 48000;
    int sourceAudioChannels = 2;
};

using BurstPayload = std::variant<AudioBurst, VideoBurst>;

class IBurstcon
{
public:
    virtual ~IBurstcon() = default;
    virtual bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) = 0;
    virtual BurstOriginInfo originInfo() const = 0;
    virtual void invalidate(const BurstRange &range) = 0;
    /// When a cache layer returns video without touching the decoder, forward the media burst index
    /// so shared demuxer "logical cursor" matches playback (avoids massive catch-up reads on miss).
    virtual void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) { (void)mediaBurstIndex; }

    bool audioBurst(const BurstRequest &request, AudioBurst &out);
    bool videoBurst(const BurstRequest &request, VideoBurst &out);
};

struct ConCacheConfig {
    size_t maxEntries = 384*384;
    size_t maxAudioEntries = 192*192;
    size_t maxVideoEntries = 256*256;
    uint64_t maxVideoBytes = 2048ULL * 1024ULL * 1024ULL;
    BurstIndex preloadAheadTicks = kBurstTimebaseHz; // +1 second
    int preloadStepHz = 30*60;
    bool preloadEnabled = true;
    /// When false, only request quality + source resolution apply (no `forced` override).
    bool forcedVideoQualityEnabled = false;
    BurstQuality forcedVideoQuality;
    /// Fires when a video frame enters or leaves this ConCache (LRU evict / replace / invalidate).
    std::function<void(bool inserted, const QString &mediaPath, BurstIndex mediaBurstIndex, const BurstQuality &q)>
        onVideoFrameCacheChanged;
};

struct BurstconMetrics {
    uint64_t avcppOpenCalls = 0;
    uint64_t avcppPacketReads = 0;
    uint64_t avcppVideoDecodeCalls = 0;
    uint64_t avcppAudioDecodeCalls = 0;
    uint64_t cacheHits = 0;
    uint64_t cacheMisses = 0;
    uint64_t cacheEntries = 0;
    /// ConCache video frames dropped by LRU or overwritten before any read (prefetch thrash).
    uint64_t cacheVideoUnusedEvictions = 0;
};

struct SharedMediaFile;

class ConCache : public IBurstcon
{
public:
    ConCache(std::shared_ptr<IBurstcon> upstream, QString mediaPath, const ConCacheConfig &config = {});
    ~ConCache() override;

    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;
    void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) override;

private:
    struct AudioKey {
        BurstIndex index = 0;
        bool operator==(const AudioKey &other) const { return index == other.index; }
    };

    struct VideoKey {
        BurstIndex index = 0;
        int width = 0;
        int height = 0;
        bool operator==(const VideoKey &other) const
        {
            return index == other.index && width == other.width && height == other.height;
        }
    };

    struct AudioKeyHash {
        size_t operator()(const AudioKey &key) const;
    };

    struct VideoKeyHash {
        size_t operator()(const VideoKey &key) const;
    };

    struct AudioEntry {
        AudioBurst value;
        std::list<AudioKey>::iterator lruIt;
        BurstIndex index = 0;
    };
    struct VideoEntry {
        VideoBurst value;
        std::list<VideoKey>::iterator lruIt;
        BurstIndex index = 0;
        bool hadHit = false;
    };

    bool getAudio(const AudioKey &key, AudioBurst &out);
    bool getVideo(const VideoKey &key, VideoBurst &out);
    void put(const AudioKey &key, const AudioBurst &value, BurstIndex index);
    void put(const VideoKey &key, const VideoBurst &value, BurstIndex index);
    void evictAudioLocked();
    void evictVideoLocked(std::vector<VideoKey> &removedVideoKeys);
    void touchAudioLocked(const AudioKey &key, AudioEntry &entry);
    void touchVideoLocked(const VideoKey &key, VideoEntry &entry);
    AudioKey keyForAudio(const BurstRequest &request) const;
    VideoKey keyForVideo(const BurstRequest &request) const;
    BurstQuality effectiveVideoQuality(const BurstRequest &request) const;
    void schedulePreload(const BurstRequest &request, bool video, bool internalRequest);
    void preloadLoop();
    void preloadBurst(BurstRequest request, bool video);
    void notifyVideoFrameCache(bool inserted, const VideoKey &key);

    std::shared_ptr<IBurstcon> upstream;
    QString mediaPath;
    ConCacheConfig config;

    mutable std::mutex mutex;
    std::unordered_map<AudioKey, AudioEntry, AudioKeyHash> audioEntries;
    std::unordered_map<VideoKey, VideoEntry, VideoKeyHash> videoEntries;
    std::list<AudioKey> audioLru;
    std::list<VideoKey> videoLru;
    uint64_t videoBytesUsed = 0;

    std::mutex preloadMutex;
    std::vector<std::pair<BurstRequest, bool>> preloadQueue;
    BurstIndex lastScheduledAudioPreload = std::numeric_limits<BurstIndex>::min();
    BurstIndex lastScheduledVideoPreload = std::numeric_limits<BurstIndex>::min();
    std::condition_variable preloadCv;
    std::atomic_bool stopPreload{false};
    std::thread preloadThread;
};

class IMediaDecoderBackend
{
public:
    virtual ~IMediaDecoderBackend() = default;
    virtual bool load(const QString &path) = 0;
    virtual BurstOriginInfo originInfo() const = 0;
    virtual bool decodeVideo(BurstIndex index,
                             BurstIndex decodeOrigin,
                             const BurstQuality &quality,
                             bool skipFrames,
                             VideoBurst &out) = 0;
    virtual bool decodeAudio(BurstIndex index,
                             BurstIndex decodeOrigin,
                             int sampleRate,
                             int channels,
                             int samplesPerBurst,
                             AudioBurst &out) = 0;
    /// Logical playback position in this file's burst timeline (does not seek the demuxer).
    virtual void setPresentationVideoIndex(BurstIndex mediaBurstIndex) { (void)mediaBurstIndex; }
};

class ConMedia : public IBurstcon
{
public:
    ConMedia(const QString &path, std::shared_ptr<IMediaDecoderBackend> backend);

    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;
    void syncPresentationVideoIndex(BurstIndex mediaBurstIndex) override;

private:
    QString path;
    std::shared_ptr<IMediaDecoderBackend> backend;
    mutable std::mutex backendMutex;
};

struct ConMediaPoolConfig {
    bool enableConMediaCache = true;
    ConCacheConfig cacheConfig;
    bool forcedPreviewQualityEnabled = false;
    BurstQuality forcedPreviewQuality;
    size_t maxMediaInstances = 24;
    /// Strip / preview video: pass `poolKeyVariant` in [1, N] to `acquire` so parallel decodes for the
    /// same path do not serialize on one `ConMedia` (avoids HEVC DPB corruption and cache thrash).
    size_t previewParallelShards = 1;
};

class ConMediaPool
{
public:
    explicit ConMediaPool(const ConMediaPoolConfig &config = {});

    std::shared_ptr<IBurstcon> acquire(const QString &path,
                                       bool forPreview,
                                       bool skipFrames,
                                       BurstIndex decodeOrigin = -1,
                                       bool wantVideo = true,
                                       bool wantAudio = true,
                                       size_t poolKeyVariant = 0);
    void invalidatePath(const QString &path);
    /// Effective shard count for `poolKeyVariant` (at least 1). Used by strip thumbnail scheduling.
    size_t previewParallelShardCount() const;

private:
    std::shared_ptr<IBurstcon> createMedia(const QString &path,
                                          bool forPreview,
                                          bool skipFrames,
                                          BurstIndex decodeOrigin,
                                          bool wantVideo,
                                          bool wantAudio);
    std::shared_ptr<SharedMediaFile> acquireSharedMediaFile(const QString &path);

    ConMediaPoolConfig config;
    std::mutex mutex;
    std::unordered_map<std::string, std::shared_ptr<IBurstcon>> mediaByKey;
    std::unordered_map<std::string, std::shared_ptr<SharedMediaFile>> sharedFilesByPath;
    std::list<std::string> mediaLru;
};

struct ConTrackEvent {
    QString sourcePath;
    BurstIndex start = 0;
    BurstIndex duration = 0;
    BurstIndex sourceIn = 0;
    bool hasVideo = false;
    bool hasAudio = false;
    int zIndex = 0;
    /// Timeline lane index (same as UI track row). Used for same-lane audio overlap crossfades.
    int timelineLaneIndex = 0;
    /// Per-clip linear gain (1 = unity). Applied after optional `audioFxChain`.
    float audioLinearGain = 1.f;
    /// `{ "type": "...", "params": { } }` entries; order is decoder → compositor.
    QJsonArray videoFxChain;
    QJsonArray audioFxChain;
};

class ConTrack : public IBurstcon
{
public:
    explicit ConTrack(std::shared_ptr<ConMediaPool> mediaPool);

    void setOutputVideoSize(BurstQuality q);
    void setEvents(const std::vector<ConTrackEvent> &events);
    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;

private:
    struct BoundEvent {
        ConTrackEvent event;
        std::shared_ptr<IBurstcon> media;
    };

    std::shared_ptr<ConMediaPool> mediaPool;
    std::vector<BoundEvent> events;
    BurstQuality outputVideoSize{1280, 720};
    mutable std::mutex mutex;
};

class ConTrimmer : public IBurstcon
{
public:
    ConTrimmer() = default;

    void setTracks(const std::vector<std::shared_ptr<ConTrack>> &tracks);
    bool burst(BurstType type, const BurstRequest &request, BurstPayload &out) override;
    BurstOriginInfo originInfo() const override;
    void invalidate(const BurstRange &range) override;

private:
    std::vector<std::shared_ptr<ConTrack>> tracks;
    mutable std::mutex mutex;
};

enum class PlaybackMode {
    Free,
    Region,
    RegionLoop
};

class MonotonicPlaybackTimer
{
public:
    struct Callbacks {
        std::function<void(const AudioBurst &)> onAudio;
        std::function<void(VideoBurst)> onVideo;
    };

    explicit MonotonicPlaybackTimer(std::shared_ptr<IBurstcon> source);
    ~MonotonicPlaybackTimer();

    void setSource(std::shared_ptr<IBurstcon> source);
    void setMode(PlaybackMode mode, BurstIndex regionStart, BurstIndex regionEnd);
    void setVideoRequestQuality(const BurstQuality &quality);
    /// Must match the project's output FPS so burst indices align with prefetch/warmup caches.
    void setPlaybackVideoFps(int fps);
    void setCallbacks(const Callbacks &callbacks);
    void start(BurstIndex startIndex);
    void stop();
    bool isRunning() const;
    BurstIndex currentIndex() const;

private:
    void runLoop();
    void preloadAhead(const std::shared_ptr<IBurstcon> &source,
                      const BurstQuality &videoQuality,
                      BurstIndex secondStart,
                      BurstIndex videoStep,
                      BurstIndex audioStep);

    mutable std::mutex mutex;
    std::shared_ptr<IBurstcon> source;
    Callbacks callbacks;
    PlaybackMode mode = PlaybackMode::Free;
    BurstIndex regionStart = 0;
    BurstIndex regionEnd = 0;
    BurstQuality videoRequestQuality;
    int playbackVideoFps = 30;
    std::atomic_bool running{false};
    std::thread timerThread;
    std::atomic<BurstIndex> index{0};
    BurstIndex lastPreloadSecond = std::numeric_limits<BurstIndex>::min();
};

int64_t msToBurstIndex(int64_t ms);
/// Smallest burst span that fully covers `ms` milliseconds (matches clip duration rounding in the editor).
BurstIndex msToBurstIndexCeil(int64_t ms);
int64_t burstIndexToMs(BurstIndex index);

/// Burst tick step for one output timeline video frame at integer `outputFps` (project / export FPS).
BurstIndex outputVideoFrameStepTicks(int outputFps);
/// Floor-align a burst index to the output-frame grid at `outputFps` (same grid as realtime playback).
BurstIndex alignBurstIndexToOutputVideoFrame(int outputFps, BurstIndex index);

BurstconMetrics metricsSnapshot();
void resetMetrics();

/// Optional UI hook (diagnostics log). Thread-safe; handler should marshal to the GUI thread.
void setBurstconDiagnosticHandler(std::function<void(const QString &)> handler);
void reportBurstconDiagnostic(const QString &message);
/// Throttled: increments `which` first; logs when `nMod` is 1 (always), or when `(which % nMod) == 1` (1,9,17,… for 8).
void reportBurstconDiagnosticEvery(const QString &message, std::atomic<uint64_t> &which, uint32_t nMod = 1);

} // namespace burstcon

#endif // BURSTCON_H
