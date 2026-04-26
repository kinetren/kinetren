#include "bc_fx.h"

#include <QPainter>
#include <QtMath>

#include <QJsonArray>
#include <QJsonObject>
#include <QList>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <functional>
#include <memory>
#include <vector>

namespace burstcon {

bool composePanZoomOutput(const QImage &srcArgb32,
                          int outW,
                          int outH,
                          const QString &mode,
                          double panX,
                          double panY,
                          double zoom,
                          QImage &outImage)
{
    if (srcArgb32.isNull() || srcArgb32.format() != QImage::Format_ARGB32) {
        return false;
    }
    outW = std::max(1, outW);
    outH = std::max(1, outH);
    zoom = std::clamp(zoom, 0.05, 32.0);
    const double sw = srcArgb32.width();
    const double sh = srcArgb32.height();
    if (sw < 1.0 || sh < 1.0) {
        return false;
    }

    double cropW = sw;
    double cropH = sh;

    const double arSrc = sw / sh;
    const double arOut = static_cast<double>(outW) / static_cast<double>(outH);

    if (mode == QStringLiteral("stretch")) {
        cropW = sw;
        cropH = sh;
    } else if (mode == QStringLiteral("fill")) {
        if (arSrc > arOut) {
            cropH = sh;
            cropW = sh * arOut;
        } else {
            cropW = sw;
            cropH = sw / arOut;
        }
    } else {
        // fit (letterbox / pillarbox)
        if (arSrc > arOut) {
            cropW = sw;
            cropH = sw / arOut;
        } else {
            cropH = sh;
            cropW = sh * arOut;
        }
    }

    const double invZoom = 1.0 / zoom;
    const double zw = cropW * invZoom;
    const double zh = cropH * invZoom;
    const double slackX = std::max(0.0, sw - zw);
    const double slackY = std::max(0.0, sh - zh);
    const double cx = sw * 0.5 + panX * slackX * 0.5;
    const double cy = sh * 0.5 + panY * slackY * 0.5;
    cropW = zw;
    cropH = zh;
    double cropX = cx - cropW * 0.5;
    double cropY = cy - cropH * 0.5;
    cropX = std::clamp(cropX, 0.0, std::max(0.0, sw - cropW));
    cropY = std::clamp(cropY, 0.0, std::max(0.0, sh - cropH));

    QImage out(outW, outH, QImage::Format_ARGB32);
    out.fill(QColor(0, 0, 0, 255));

    QPainter painter(&out);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
    const QRectF dest(0, 0, outW, outH);
    const QRectF source(cropX, cropY, cropW, cropH);
    painter.drawImage(dest, srcArgb32, source);
    painter.end();

    outImage = std::move(out);
    return true;
}

namespace {

QJsonObject mergeDefaults(const QJsonObject &base, const QJsonObject &over)
{
    QJsonObject out = base;
    for (auto it = over.begin(); it != over.end(); ++it) {
        out.insert(it.key(), it.value());
    }
    return out;
}

// --- VideoPanFx ---

class VideoPanFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("pan"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaultParams(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &ctx) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const int outW = std::max(1, ctx.projectOutput.width);
        const int outH = std::max(1, ctx.projectOutput.height);
        const QString mode = p.value(QStringLiteral("mode")).toString(QStringLiteral("fit"));
        const double panX = p.value(QStringLiteral("panX")).toDouble(0.0);
        const double panY = p.value(QStringLiteral("panY")).toDouble(0.0);
        const double zoom = std::clamp(p.value(QStringLiteral("zoom")).toDouble(1.0), 0.05, 32.0);

        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        QImage out;
        if (!composePanZoomOutput(src, outW, outH, mode, panX, panY, zoom, out)) {
            return false;
        }
        inOut.quality = BurstQuality{outW, outH};
        inOut.frame = std::move(out);
        return true;
    }

private:
    static QJsonObject defaultParams()
    {
        QJsonObject o;
        o.insert(QStringLiteral("mode"), QStringLiteral("fit"));
        o.insert(QStringLiteral("panX"), 0.0);
        o.insert(QStringLiteral("panY"), 0.0);
        o.insert(QStringLiteral("zoom"), 1.0);
        return o;
    }

    QJsonObject p = defaultParams();
};

struct MotionKf {
    double t = 0.0;
    double panX = 0.0;
    double panY = 0.0;
    double zoom = 1.0;
};

static bool motionKfLess(const MotionKf &a, const MotionKf &b)
{
    return a.t < b.t;
}

static double smoothstep01(double x)
{
    x = std::clamp(x, 0.0, 1.0);
    return x * x * (3.0 - 2.0 * x);
}

static void parseMotionKeyframes(const QJsonObject &params, std::vector<MotionKf> &out)
{
    out.clear();
    const QJsonArray arr = params.value(QStringLiteral("keyframes")).toArray();
    for (const QJsonValue &v : arr) {
        if (!v.isObject()) {
            continue;
        }
        const QJsonObject o = v.toObject();
        MotionKf k;
        k.t = std::clamp(o.value(QStringLiteral("t")).toDouble(0.0), 0.0, 1.0);
        k.panX = std::clamp(o.value(QStringLiteral("panX")).toDouble(0.0), -1.0, 1.0);
        k.panY = std::clamp(o.value(QStringLiteral("panY")).toDouble(0.0), -1.0, 1.0);
        k.zoom = std::clamp(o.value(QStringLiteral("zoom")).toDouble(1.0), 0.05, 32.0);
        out.push_back(k);
    }
    if (out.empty()) {
        out.push_back(MotionKf{0.0, 0.0, 0.0, 1.0});
        out.push_back(MotionKf{1.0, 0.0, 0.0, 1.0});
        return;
    }
    std::sort(out.begin(), out.end(), motionKfLess);
    // De-duplicate by t (keep last)
    std::vector<MotionKf> dedup;
    dedup.reserve(out.size());
    for (const MotionKf &k : out) {
        if (!dedup.empty() && std::abs(dedup.back().t - k.t) < 1e-9) {
            dedup.back() = k;
        } else {
            dedup.push_back(k);
        }
    }
    out = std::move(dedup);
}

static void sampleMotionAt(const std::vector<MotionKf> &kfs,
                           double u01,
                           bool easeSegments,
                           double &panX,
                           double &panY,
                           double &zoom)
{
    if (kfs.empty()) {
        panX = panY = 0.0;
        zoom = 1.0;
        return;
    }
    u01 = std::clamp(u01, 0.0, 1.0);
    if (u01 <= kfs.front().t) {
        panX = kfs.front().panX;
        panY = kfs.front().panY;
        zoom = kfs.front().zoom;
        return;
    }
    if (u01 >= kfs.back().t) {
        panX = kfs.back().panX;
        panY = kfs.back().panY;
        zoom = kfs.back().zoom;
        return;
    }
    size_t i = 0;
    while (i + 1 < kfs.size() && !(u01 >= kfs[i].t && u01 <= kfs[i + 1].t)) {
        ++i;
    }
    if (i + 1 >= kfs.size()) {
        panX = kfs.back().panX;
        panY = kfs.back().panY;
        zoom = kfs.back().zoom;
        return;
    }
    const MotionKf &a = kfs[i];
    const MotionKf &b = kfs[i + 1];
    const double span = b.t - a.t;
    double s = span > 1e-12 ? (u01 - a.t) / span : 0.0;
    s = std::clamp(s, 0.0, 1.0);
    if (easeSegments) {
        s = smoothstep01(s);
    }
    panX = a.panX + (b.panX - a.panX) * s;
    panY = a.panY + (b.panY - a.panY) * s;
    zoom = a.zoom + (b.zoom - a.zoom) * s;
}

// --- Camera motion (keyframes 0..1 along clip) — copy JSON between clips for same relative move ---

class CamMotionVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("cammotion"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaultParams(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &ctx) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const int outW = std::max(1, ctx.projectOutput.width);
        const int outH = std::max(1, ctx.projectOutput.height);
        const QString mode = p.value(QStringLiteral("mode")).toString(QStringLiteral("fit"));
        const bool ease = p.value(QStringLiteral("ease")).toBool(true);
        const double timeScale = std::clamp(p.value(QStringLiteral("timeScale")).toDouble(1.0), 0.05, 8.0);

        double u = 0.0;
        if (ctx.clipDurationTicks > 0 && ctx.decodeOrigin >= 0) {
            const double rel = static_cast<double>(ctx.timelineBurstIndex - ctx.decodeOrigin);
            u = rel / static_cast<double>(ctx.clipDurationTicks);
        }
        u = std::clamp(u * timeScale, 0.0, 1.0);

        std::vector<MotionKf> kfs;
        parseMotionKeyframes(p, kfs);
        double panX = 0.0;
        double panY = 0.0;
        double zoom = 1.0;
        sampleMotionAt(kfs, u, ease, panX, panY, zoom);

        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        QImage out;
        if (!composePanZoomOutput(src, outW, outH, mode, panX, panY, zoom, out)) {
            return false;
        }
        inOut.quality = BurstQuality{outW, outH};
        inOut.frame = std::move(out);
        return true;
    }

private:
    static QJsonObject defaultParams()
    {
        QJsonObject o;
        o.insert(QStringLiteral("mode"), QStringLiteral("fit"));
        o.insert(QStringLiteral("ease"), true);
        o.insert(QStringLiteral("timeScale"), 1.0);
        QJsonArray kf;
        {
            QJsonObject a;
            a.insert(QStringLiteral("t"), 0.0);
            a.insert(QStringLiteral("panX"), 0.0);
            a.insert(QStringLiteral("panY"), 0.0);
            a.insert(QStringLiteral("zoom"), 1.0);
            kf.append(a);
        }
        {
            QJsonObject b;
            b.insert(QStringLiteral("t"), 1.0);
            b.insert(QStringLiteral("panX"), 0.0);
            b.insert(QStringLiteral("panY"), 0.0);
            b.insert(QStringLiteral("zoom"), 1.35);
            kf.append(b);
        }
        o.insert(QStringLiteral("keyframes"), kf);
        return o;
    }

    QJsonObject p = defaultParams();
};

// --- Mirror ---

class MirrorVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("mirror"); }
    void setParams(const QJsonObject &o) override { p = o; }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const bool h = p.value(QStringLiteral("horizontal")).toBool(true);
        const bool v = p.value(QStringLiteral("vertical")).toBool(false);
        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        QTransform t;
        if (h) {
            t.scale(-1, 1);
        }
        if (v) {
            t.scale(h ? -1 : 1, -1);
        }
        if (!h && !v) {
            return true;
        }
        QImage flipped = src.transformed(t, Qt::SmoothTransformation);
        inOut.frame = flipped;
        inOut.quality = BurstQuality{flipped.width(), flipped.height()};
        return true;
    }

private:
    QJsonObject p;
};

// --- Blur (cheap resize) ---

class BlurVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("blur"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const int radius = std::clamp(p.value(QStringLiteral("radius")).toInt(8), 1, 48);
        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        const int w = src.width();
        const int h = src.height();
        const int step = std::max(1, radius / 3);
        const int tw = std::max(1, w / step);
        const int th = std::max(1, h / step);
        QImage small = src.scaled(tw, th, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        QImage blurred = small.scaled(w, h, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        inOut.frame = blurred;
        inOut.quality = BurstQuality{w, h};
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("radius"), 8);
        return o;
    }

    QJsonObject p = defaults();
};

// --- Greenscreen / chroma ---

static QRgb sampleKey(const QImage &img, const QJsonObject &p)
{
    if (p.contains(QStringLiteral("keyR"))) {
        return qRgb(p.value(QStringLiteral("keyR")).toInt(0),
                    p.value(QStringLiteral("keyG")).toInt(255),
                    p.value(QStringLiteral("keyB")).toInt(0));
    }
    return qRgb(0, 255, 0);
}

class GreenscreenVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("greenscreen"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        QImage img = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        const QRgb key = sampleKey(img, p);
        const int kr = qRed(key);
        const int kg = qGreen(key);
        const int kb = qBlue(key);
        const int tol = std::clamp(p.value(QStringLiteral("tolerance")).toInt(60), 1, 200);
        const int soft = std::clamp(p.value(QStringLiteral("softness")).toInt(20), 0, 100);
        for (int y = 0; y < img.height(); ++y) {
            QRgb *line = reinterpret_cast<QRgb *>(img.scanLine(y));
            for (int x = 0; x < img.width(); ++x) {
                const QRgb c = line[x];
                const int dr = qRed(c) - kr;
                const int dg = qGreen(c) - kg;
                const int db = qBlue(c) - kb;
                const int dist = static_cast<int>(std::sqrt(double(dr * dr + dg * dg + db * db)));
                int alpha = qAlpha(c);
                if (dist <= tol - soft) {
                    alpha = 0;
                } else if (soft > 0 && dist < tol) {
                    const double t = static_cast<double>(dist - (tol - soft)) / static_cast<double>(std::max(1, soft));
                    alpha = static_cast<int>(alpha * std::clamp(t, 0.0, 1.0));
                }
                line[x] = qRgba(qRed(c), qGreen(c), qBlue(c), alpha);
            }
        }
        inOut.frame = img;
        inOut.quality = BurstQuality{img.width(), img.height()};
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("keyR"), 0);
        o.insert(QStringLiteral("keyG"), 255);
        o.insert(QStringLiteral("keyB"), 0);
        o.insert(QStringLiteral("tolerance"), 70);
        o.insert(QStringLiteral("softness"), 25);
        return o;
    }

    QJsonObject p = defaults();
};

// --- Edges ---

class EdgesVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("edges"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const int thresh = std::clamp(p.value(QStringLiteral("threshold")).toInt(40), 1, 255);
        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        const int w = src.width();
        const int h = src.height();
        QImage out(w, h, QImage::Format_ARGB32);
        auto lum = [&](int x, int y) -> int {
            if (x < 0 || y < 0 || x >= w || y >= h) {
                return 0;
            }
            const QRgb c = src.pixel(x, y);
            return (77 * qRed(c) + 150 * qGreen(c) + 29 * qBlue(c)) >> 8;
        };
        for (int y = 1; y < h - 1; ++y) {
            QRgb *dst = reinterpret_cast<QRgb *>(out.scanLine(y));
            for (int x = 1; x < w - 1; ++x) {
                const int gx = -lum(x - 1, y - 1) + lum(x + 1, y - 1) - 2 * lum(x - 1, y) + 2 * lum(x + 1, y) -
                    lum(x - 1, y + 1) + lum(x + 1, y + 1);
                const int gy = -lum(x - 1, y - 1) - 2 * lum(x, y - 1) - lum(x + 1, y - 1) + lum(x - 1, y + 1) +
                    2 * lum(x, y + 1) + lum(x + 1, y + 1);
                const int mag = static_cast<int>(std::min<double>(255.0, std::sqrt(double(gx * gx + gy * gy))));
                const int e = mag >= thresh ? mag : 0;
                dst[x] = qRgba(e, e, e, 255);
            }
        }
        inOut.frame = out;
        inOut.quality = BurstQuality{w, h};
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("threshold"), 48);
        return o;
    }

    QJsonObject p = defaults();
};

// --- Noise ---

class NoiseVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("noise"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &ctx) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const double amount = std::clamp(p.value(QStringLiteral("amount")).toDouble(0.08), 0.0, 1.0);
        QImage img = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        const qint64 seed = ctx.timelineBurstIndex * 1103515245LL + 12345;
        quint32 rng = static_cast<quint32>(seed);
        auto rnd = [&]() {
            rng = rng * 1664525u + 1013904223u;
            return static_cast<int>(rng & 0xffff) - 0x7fff;
        };
        for (int y = 0; y < img.height(); ++y) {
            QRgb *line = reinterpret_cast<QRgb *>(img.scanLine(y));
            for (int x = 0; x < img.width(); ++x) {
                const QRgb c = line[x];
                const int n = rnd();
                const int k = static_cast<int>(amount * 32);
                line[x] = qRgba(std::clamp(qRed(c) + (n * k) / 32767, 0, 255),
                                std::clamp(qGreen(c) + (n * k) / 32767, 0, 255),
                                std::clamp(qBlue(c) + (n * k) / 32767, 0, 255),
                                qAlpha(c));
            }
        }
        inOut.frame = img;
        inOut.quality = BurstQuality{img.width(), img.height()};
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("amount"), 0.08);
        return o;
    }

    QJsonObject p = defaults();
};

// --- VHS ---

class VhsVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("vhs"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &ctx) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const int shift = std::clamp(p.value(QStringLiteral("rgbShift")).toInt(3), 0, 12);
        const double bleed = std::clamp(p.value(QStringLiteral("bleed")).toDouble(0.35), 0.0, 1.0);
        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        const int w = src.width();
        const int h = src.height();
        QImage out = src.copy();
        const int lineOff = static_cast<int>((ctx.timelineBurstIndex / 3000) % 7);
        for (int y = 0; y < h; ++y) {
            const int scan = ((y + lineOff) % 4 == 0) ? 1 : 0;
            QRgb *dst = reinterpret_cast<QRgb *>(out.scanLine(y));
            const QRgb *sr0 = reinterpret_cast<const QRgb *>(src.constScanLine(y));
            for (int x = 0; x < w; ++x) {
                const int xr = std::clamp(x + shift + scan, 0, w - 1);
                const int xb = std::clamp(x - shift - scan, 0, w - 1);
                const QRgb a = sr0[x];
                const QRgb rpix = src.pixel(xr, y);
                const QRgb bpix = src.pixel(xb, y);
                int rr = static_cast<int>(qRed(a) * (1 - bleed) + qRed(rpix) * bleed);
                int gg = qGreen(a);
                int bb = static_cast<int>(qBlue(a) * (1 - bleed) + qBlue(bpix) * bleed);
                if (scan) {
                    gg = std::clamp(gg - 18, 0, 255);
                }
                dst[x] = qRgba(rr, gg, bb, qAlpha(a));
            }
        }
        inOut.frame = out;
        inOut.quality = BurstQuality{w, h};
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("rgbShift"), 4);
        o.insert(QStringLiteral("bleed"), 0.4);
        return o;
    }

    QJsonObject p = defaults();
};

// --- Glow ---

class GlowVideoFx : public IVideoFx
{
public:
    QString typeId() const override { return QStringLiteral("glow"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(VideoBurst &inOut, const VideoFxContext &) override
    {
        if (inOut.frame.isNull()) {
            return false;
        }
        const double strength = std::clamp(p.value(QStringLiteral("strength")).toDouble(0.6), 0.0, 2.0);
        QImage src = inOut.frame.convertToFormat(QImage::Format_ARGB32);
        const int w = src.width();
        const int h = src.height();
        const int tw = std::max(1, w / 4);
        const int th = std::max(1, h / 4);
        QImage blur = src.scaled(tw, th, Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
                         .scaled(w, h, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        QImage out = src.copy();
        for (int y = 0; y < h; ++y) {
            QRgb *d = reinterpret_cast<QRgb *>(out.scanLine(y));
            const QRgb *b = reinterpret_cast<const QRgb *>(blur.constScanLine(y));
            const QRgb *s = reinterpret_cast<const QRgb *>(src.constScanLine(y));
            for (int x = 0; x < w; ++x) {
                const int r = std::min(255, static_cast<int>(qRed(s[x]) + qRed(b[x]) * strength * 0.35));
                const int g = std::min(255, static_cast<int>(qGreen(s[x]) + qGreen(b[x]) * strength * 0.35));
                const int bl = std::min(255, static_cast<int>(qBlue(s[x]) + qBlue(b[x]) * strength * 0.35));
                d[x] = qRgba(r, g, bl, qAlpha(s[x]));
            }
        }
        inOut.frame = out;
        inOut.quality = BurstQuality{w, h};
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("strength"), 0.65);
        return o;
    }

    QJsonObject p = defaults();
};

// --- Audio: soft clip ---

class SoftClipAudioFx : public IAudioFx
{
public:
    QString typeId() const override { return QStringLiteral("softclip"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(AudioBurst &inOut, const AudioFxContext &) override
    {
        const double drive = std::clamp(p.value(QStringLiteral("drive")).toDouble(1.15), 0.5, 4.0);
        for (auto &s : inOut.samples) {
            const double x = static_cast<double>(s) * drive / 2147483648.0;
            const double t = std::tanh(x);
            s = static_cast<int32_t>(std::clamp(t * 2147483647.0,
                                                static_cast<double>(std::numeric_limits<int32_t>::min()),
                                                static_cast<double>(std::numeric_limits<int32_t>::max())));
        }
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("drive"), 1.2);
        return o;
    }

    QJsonObject p = defaults();
};

// --- Audio: one-pole lowpass ---

class LowpassAudioFx : public IAudioFx
{
public:
    QString typeId() const override { return QStringLiteral("lowpass"); }
    void setParams(const QJsonObject &o) override { p = mergeDefaults(defaults(), o); }
    QJsonObject params() const override { return p; }

    bool process(AudioBurst &inOut, const AudioFxContext &ctx) override
    {
        const int ch = std::max(1, inOut.channels > 0 ? inOut.channels : ctx.channels);
        if (static_cast<int>(z.size()) < ch) {
            z.assign(static_cast<size_t>(ch), 0.0);
        }
        const double cutoff = std::clamp(p.value(QStringLiteral("cutoffHz")).toDouble(8000.0), 100.0, 18000.0);
        const int rate = std::max(1, inOut.sampleRate > 0 ? inOut.sampleRate : ctx.sampleRate);
        const double dt = 1.0 / static_cast<double>(rate);
        constexpr double kPi = 3.14159265358979323846;
        const double rc = 1.0 / (2.0 * kPi * cutoff);
        const double alpha = dt / (rc + dt);
        for (size_t i = 0; i < inOut.samples.size(); ++i) {
            const int c = static_cast<int>(i % static_cast<size_t>(ch));
            double x = static_cast<double>(inOut.samples[i]);
            z[static_cast<size_t>(c)] = alpha * x + (1.0 - alpha) * z[static_cast<size_t>(c)];
            inOut.samples[i] = static_cast<int32_t>(std::clamp(z[static_cast<size_t>(c)],
                                                               static_cast<double>(std::numeric_limits<int32_t>::min()),
                                                               static_cast<double>(std::numeric_limits<int32_t>::max())));
        }
        return true;
    }

private:
    static QJsonObject defaults()
    {
        QJsonObject o;
        o.insert(QStringLiteral("cutoffHz"), 6000.0);
        return o;
    }

    QJsonObject p = defaults();
    std::vector<double> z;
};

} // namespace

static double rgbMseSubsampled(const QImage &a, const QImage &b)
{
    if (a.isNull() || b.isNull() || a.size() != b.size()) {
        return 1e18;
    }
    const QImage aa = a.convertToFormat(QImage::Format_ARGB32);
    const QImage bb = b.convertToFormat(QImage::Format_ARGB32);
    const int w = aa.width();
    const int h = aa.height();
    if (w < 1 || h < 1) {
        return 1e18;
    }
    const int step = (std::min(w, h) >= 160) ? 2 : 1;
    double sum = 0.0;
    int n = 0;
    for (int y = 0; y < h; y += step) {
        const QRgb *la = reinterpret_cast<const QRgb *>(aa.constScanLine(y));
        const QRgb *lb = reinterpret_cast<const QRgb *>(bb.constScanLine(y));
        for (int x = 0; x < w; x += step) {
            const QRgb ca = la[x];
            const QRgb cb = lb[x];
            const int dr = int(qRed(ca)) - int(qRed(cb));
            const int dg = int(qGreen(ca)) - int(qGreen(cb));
            const int db = int(qBlue(ca)) - int(qBlue(cb));
            sum += double(dr * dr + dg * dg + db * db);
            ++n;
        }
    }
    return n > 0 ? sum / double(n) : 1e18;
}

static void searchPanZoomMatch(const QImage &refArgb,
                               const QImage &targetArgb,
                               int outW,
                               int outH,
                               const QString &mode,
                               double hintPanX,
                               double hintPanY,
                               double hintZoom,
                               double &outPanX,
                               double &outPanY,
                               double &outZoom)
{
    const int maxW = 96;
    QImage refS = refArgb.convertToFormat(QImage::Format_ARGB32);
    if (refS.width() > maxW) {
        refS = refS.scaledToWidth(maxW, Qt::SmoothTransformation);
    }
    refS = refS.convertToFormat(QImage::Format_ARGB32);
    const int outSw = std::max(32, refS.width());
    const int outSh = std::max(18, (outSw * outH + outW / 2) / std::max(1, outW));

    QImage targS =
        targetArgb.scaled(outSw, outSh, Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
            .convertToFormat(QImage::Format_ARGB32);

    auto eval = [&](double panX, double panY, double zoom) -> double {
        QImage syn;
        if (!composePanZoomOutput(refS, outSw, outSh, mode, panX, panY, zoom, syn)) {
            return 1e18;
        }
        return rgbMseSubsampled(syn, targS);
    };

    double best = 1e18;
    outPanX = hintPanX;
    outPanY = hintPanY;
    outZoom = hintZoom;

    // Wide coarse search (strong zoom-ins / push beyond frame edges need high zoom values).
    const double zMin = 0.22;
    const double zMax = 6.5;
    const int nz = 22;
    const int np = 13;
    for (int iz = 0; iz < nz; ++iz) {
        const double t = (nz <= 1) ? 0.5 : double(iz) / double(nz - 1);
        const double zoom = zMin + (zMax - zMin) * t;
        for (int ix = 0; ix < np; ++ix) {
            const double panX = -1.0 + (2.0 * double(ix)) / double(np - 1);
            for (int iy = 0; iy < np; ++iy) {
                const double panY = -1.0 + (2.0 * double(iy)) / double(np - 1);
                const double m = eval(panX, panY, zoom);
                if (m < best) {
                    best = m;
                    outPanX = panX;
                    outPanY = panY;
                    outZoom = zoom;
                }
            }
        }
    }

    // Local refine
    for (int pass = 0; pass < 4; ++pass) {
        const double stepPan = 0.06 / double(1 << pass);
        const double stepZ = 0.045 / double(1 << pass);
        for (int dz = -2; dz <= 2; ++dz) {
            for (int dx = -2; dx <= 2; ++dx) {
                for (int dy = -2; dy <= 2; ++dy) {
                    if (dx == 0 && dy == 0 && dz == 0) {
                        continue;
                    }
                    const double panX = std::clamp(outPanX + dx * stepPan, -1.0, 1.0);
                    const double panY = std::clamp(outPanY + dy * stepPan, -1.0, 1.0);
                    const double zoom = std::clamp(outZoom + dz * stepZ, 0.08, 8.0);
                    const double m = eval(panX, panY, zoom);
                    if (m < best) {
                        best = m;
                        outPanX = panX;
                        outPanY = panY;
                        outZoom = zoom;
                    }
                }
            }
        }
    }

    // Full-res polish (few evaluations)
    QImage refF = refArgb.convertToFormat(QImage::Format_ARGB32);
    QImage targF = targetArgb.convertToFormat(QImage::Format_ARGB32);
    if (targF.size() != QSize(outW, outH)) {
        targF = targF.scaled(outW, outH, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
    }
    auto evalFull = [&](double panX, double panY, double zoom) -> double {
        QImage syn;
        if (!composePanZoomOutput(refF, outW, outH, mode, panX, panY, zoom, syn)) {
            return 1e18;
        }
        return rgbMseSubsampled(syn, targF);
    };
    for (int pass = 0; pass < 3; ++pass) {
        const double sp = 0.025 / double(1 << pass);
        const double sz = 0.02 / double(1 << pass);
        for (int dz = -1; dz <= 1; ++dz) {
            for (int dx = -1; dx <= 1; ++dx) {
                for (int dy = -1; dy <= 1; ++dy) {
                    if (dx == 0 && dy == 0 && dz == 0) {
                        continue;
                    }
                    const double panX = std::clamp(outPanX + dx * sp, -1.0, 1.0);
                    const double panY = std::clamp(outPanY + dy * sp, -1.0, 1.0);
                    const double zoom = std::clamp(outZoom + dz * sz, 0.08, 8.0);
                    const double m = evalFull(panX, panY, zoom);
                    if (m < best) {
                        best = m;
                        outPanX = panX;
                        outPanY = panY;
                        outZoom = zoom;
                    }
                }
            }
        }
    }
}

struct EstMotionKf {
    double t = 0.0;
    double panX = 0.0;
    double panY = 0.0;
    double zoom = 1.0;
};

static double chordDeviation(const EstMotionKf &A, const EstMotionKf &B, const EstMotionKf &P)
{
    if (std::abs(B.t - A.t) < 1e-9) {
        const double dx = P.panX - A.panX;
        const double dy = P.panY - A.panY;
        const double lz = std::log(std::max(0.05, P.zoom)) - std::log(std::max(0.05, A.zoom));
        return std::sqrt(dx * dx + dy * dy + lz * lz * 2.25);
    }
    const double s = std::clamp((P.t - A.t) / (B.t - A.t), 0.0, 1.0);
    const double ix = A.panX + (B.panX - A.panX) * s;
    const double iy = A.panY + (B.panY - A.panY) * s;
    const double iz = std::log(std::max(0.05, A.zoom)) + (std::log(std::max(0.05, B.zoom)) - std::log(std::max(0.05, A.zoom))) * s;
    const double dx = P.panX - ix;
    const double dy = P.panY - iy;
    const double lz = std::log(std::max(0.05, P.zoom)) - iz;
    return std::sqrt(dx * dx + dy * dy + lz * lz * 2.25);
}

static void rdpSimplifyKeyframes(std::vector<EstMotionKf> &kf, double epsilon)
{
    if (kf.size() < 3) {
        return;
    }
    const int n = static_cast<int>(kf.size());
    std::vector<uint8_t> keep(static_cast<size_t>(n), 0);
    keep[0] = 1;
    keep[static_cast<size_t>(n - 1)] = 1;

    std::function<void(int, int)> recur;
    recur = [&](int a, int b) {
        if (b <= a + 1) {
            return;
        }
        int worst = -1;
        double maxD = 0.0;
        for (int i = a + 1; i < b; ++i) {
            const double d = chordDeviation(kf[static_cast<size_t>(a)], kf[static_cast<size_t>(b)], kf[static_cast<size_t>(i)]);
            if (d > maxD) {
                maxD = d;
                worst = i;
            }
        }
        if (worst > 0 && maxD > epsilon) {
            keep[static_cast<size_t>(worst)] = 1;
            recur(a, worst);
            recur(worst, b);
        }
    };
    recur(0, n - 1);

    std::vector<EstMotionKf> out;
    out.reserve(static_cast<size_t>(n));
    for (int i = 0; i < n; ++i) {
        if (keep[static_cast<size_t>(i)]) {
            out.push_back(kf[static_cast<size_t>(i)]);
        }
    }
    kf = std::move(out);
}

QJsonArray estimateCamMotionFromDecodedFrames(const QList<QImage> &frames,
                                              int outW,
                                              int outH,
                                              const QString &mode)
{
    QJsonArray arr;
    if (frames.size() < 2 || outW < 8 || outH < 8) {
        return arr;
    }
    QImage ref0 = frames.front().convertToFormat(QImage::Format_ARGB32);
    if (ref0.isNull()) {
        return arr;
    }
    if (ref0.size() != QSize(outW, outH)) {
        ref0 = ref0.scaled(outW, outH, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
    }

    const int n = frames.size();
    std::vector<EstMotionKf> kf;
    kf.reserve(static_cast<size_t>(n));
    double prevPanX = 0.0;
    double prevPanY = 0.0;
    double prevZoom = 1.0;

    for (int i = 0; i < n; ++i) {
        QImage tgt = frames.at(i).convertToFormat(QImage::Format_ARGB32);
        if (tgt.isNull()) {
            return {};
        }
        if (tgt.size() != QSize(outW, outH)) {
            tgt = tgt.scaled(outW, outH, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        }
        EstMotionKf row;
        row.t = (n <= 1) ? 0.0 : double(i) / double(n - 1);
        if (i == 0) {
            row.panX = 0.0;
            row.panY = 0.0;
            row.zoom = 1.0;
        } else {
            searchPanZoomMatch(ref0, tgt, outW, outH, mode, prevPanX, prevPanY, prevZoom, row.panX, row.panY, row.zoom);
        }
        row.panX = std::clamp(row.panX, -1.0, 1.0);
        row.panY = std::clamp(row.panY, -1.0, 1.0);
        row.zoom = std::clamp(row.zoom, 0.05, 32.0);
        prevPanX = row.panX;
        prevPanY = row.panY;
        prevZoom = row.zoom;
        kf.push_back(row);
    }

    rdpSimplifyKeyframes(kf, 0.055);

    for (const EstMotionKf &row : kf) {
        QJsonObject o;
        o.insert(QStringLiteral("t"), row.t);
        o.insert(QStringLiteral("panX"), row.panX);
        o.insert(QStringLiteral("panY"), row.panY);
        o.insert(QStringLiteral("zoom"), row.zoom);
        arr.append(o);
    }
    return arr;
}

// --- ConVideoFx / ConAudioFx ---

ConVideoFx::ConVideoFx(std::shared_ptr<IVideoFx> effect,
                       std::shared_ptr<IBurstcon> upstream,
                       BurstQuality projectOutput)
    : effect(std::move(effect))
    , upstream(std::move(upstream))
    , projectOutput(projectOutput)
{
}

bool ConVideoFx::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    if (!upstream || !effect) {
        return false;
    }
    if (type != BurstType::Video) {
        return upstream->burst(type, request, out);
    }
    VideoBurst vb;
    if (!upstream->videoBurst(request, vb) || vb.frame.isNull()) {
        return false;
    }
    BurstQuality outQ = request.quality;
    if (outQ.width <= 0 || outQ.height <= 0) {
        outQ = projectOutput;
    }
    VideoFxContext ctx{outQ, request.index, upstream->originInfo(), request.clipDurationTicks, request.decodeOrigin};
    if (!effect->process(vb, ctx)) {
        return false;
    }
    vb.index = request.index;
    out = std::move(vb);
    return true;
}

BurstOriginInfo ConVideoFx::originInfo() const
{
    if (upstream) {
        return upstream->originInfo();
    }
    return BurstOriginInfo{};
}

void ConVideoFx::invalidate(const BurstRange &range)
{
    if (upstream) {
        upstream->invalidate(range);
    }
}

void ConVideoFx::syncPresentationVideoIndex(BurstIndex mediaBurstIndex)
{
    if (upstream) {
        upstream->syncPresentationVideoIndex(mediaBurstIndex);
    }
}

ConAudioFx::ConAudioFx(std::shared_ptr<IAudioFx> effect, std::shared_ptr<IBurstcon> upstream)
    : effect(std::move(effect))
    , upstream(std::move(upstream))
{
}

bool ConAudioFx::burst(BurstType type, const BurstRequest &request, BurstPayload &out)
{
    if (!upstream || !effect) {
        return false;
    }
    if (type != BurstType::Audio) {
        return upstream->burst(type, request, out);
    }
    AudioBurst ab;
    if (!upstream->audioBurst(request, ab)) {
        return false;
    }
    AudioFxContext ctx{request.index, ab.sampleRate, ab.channels};
    if (!effect->process(ab, ctx)) {
        return false;
    }
    ab.index = request.index;
    out = std::move(ab);
    return true;
}

BurstOriginInfo ConAudioFx::originInfo() const
{
    if (upstream) {
        return upstream->originInfo();
    }
    return BurstOriginInfo{};
}

void ConAudioFx::invalidate(const BurstRange &range)
{
    if (upstream) {
        upstream->invalidate(range);
    }
}

void ConAudioFx::syncPresentationVideoIndex(BurstIndex mediaBurstIndex)
{
    if (upstream) {
        upstream->syncPresentationVideoIndex(mediaBurstIndex);
    }
}

std::shared_ptr<IVideoFx> createVideoFx(const QString &typeId, const QJsonObject &params)
{
    std::shared_ptr<IVideoFx> fx;
    if (typeId == QStringLiteral("pan")) {
        fx = std::make_shared<VideoPanFx>();
    } else if (typeId == QStringLiteral("cammotion")) {
        fx = std::make_shared<CamMotionVideoFx>();
    } else if (typeId == QStringLiteral("mirror")) {
        fx = std::make_shared<MirrorVideoFx>();
    } else if (typeId == QStringLiteral("blur")) {
        fx = std::make_shared<BlurVideoFx>();
    } else if (typeId == QStringLiteral("greenscreen")) {
        fx = std::make_shared<GreenscreenVideoFx>();
    } else if (typeId == QStringLiteral("edges")) {
        fx = std::make_shared<EdgesVideoFx>();
    } else if (typeId == QStringLiteral("noise")) {
        fx = std::make_shared<NoiseVideoFx>();
    } else if (typeId == QStringLiteral("vhs")) {
        fx = std::make_shared<VhsVideoFx>();
    } else if (typeId == QStringLiteral("glow")) {
        fx = std::make_shared<GlowVideoFx>();
    }
    if (fx) {
        fx->setParams(params);
    }
    return fx;
}

std::shared_ptr<IAudioFx> createAudioFx(const QString &typeId, const QJsonObject &params)
{
    std::shared_ptr<IAudioFx> fx;
    if (typeId == QStringLiteral("softclip")) {
        fx = std::make_shared<SoftClipAudioFx>();
    } else if (typeId == QStringLiteral("lowpass")) {
        fx = std::make_shared<LowpassAudioFx>();
    }
    if (fx) {
        fx->setParams(params);
    }
    return fx;
}

std::shared_ptr<IBurstcon> wrapVideoFxChain(std::shared_ptr<IBurstcon> media,
                                            const QJsonArray &chain,
                                            const BurstQuality &projectOutput)
{
    if (!media) {
        return nullptr;
    }
    std::shared_ptr<IBurstcon> node = std::move(media);
    const int n = chain.size();
    for (int i = 0; i < n; ++i) {
        const QJsonObject slot = chain.at(i).toObject();
        const QString type = slot.value(QStringLiteral("type")).toString();
        const QJsonObject params = slot.value(QStringLiteral("params")).toObject();
        std::shared_ptr<IVideoFx> fx = createVideoFx(type, params);
        if (!fx) {
            continue;
        }
        node = std::make_shared<ConVideoFx>(std::move(fx), std::move(node), projectOutput);
    }
    return node;
}

QJsonArray defaultVideoFxChain()
{
    QJsonObject panSlot;
    panSlot.insert(QStringLiteral("type"), QStringLiteral("pan"));
    panSlot.insert(QStringLiteral("params"), QJsonObject());
    return QJsonArray{panSlot};
}

bool processVideoFxChainInPlace(VideoBurst &vb, const QJsonArray &chain, const VideoFxContext &ctx)
{
    if (vb.frame.isNull()) {
        return false;
    }
    const int n = chain.size();
    for (int i = 0; i < n; ++i) {
        const QJsonObject slot = chain.at(i).toObject();
        const QString type = slot.value(QStringLiteral("type")).toString();
        const QJsonObject params = slot.value(QStringLiteral("params")).toObject();
        std::shared_ptr<IVideoFx> fx = createVideoFx(type, params);
        if (!fx) {
            continue;
        }
        if (!fx->process(vb, ctx)) {
            return false;
        }
    }
    return true;
}

std::shared_ptr<IBurstcon> wrapAudioFxChain(std::shared_ptr<IBurstcon> media, const QJsonArray &chain)
{
    if (!media) {
        return nullptr;
    }
    std::shared_ptr<IBurstcon> node = std::move(media);
    const int n = chain.size();
    for (int i = 0; i < n; ++i) {
        const QJsonObject slot = chain.at(i).toObject();
        const QString type = slot.value(QStringLiteral("type")).toString();
        const QJsonObject params = slot.value(QStringLiteral("params")).toObject();
        std::shared_ptr<IAudioFx> fx = createAudioFx(type, params);
        if (!fx) {
            continue;
        }
        node = std::make_shared<ConAudioFx>(std::move(fx), std::move(node));
    }
    return node;
}

} // namespace burstcon
