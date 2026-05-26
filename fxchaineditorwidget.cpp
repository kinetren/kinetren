#include "fxchaineditorwidget.h"
#include "timelineeditor.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>

#include <QApplication>
#include <QGuiApplication>
#include <QClipboard>
#include <QCursor>
#include <QMessageBox>
#include <QCheckBox>
#include <QColorDialog>
#include <QComboBox>
#include <QDrag>
#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QFormLayout>
#include <QFrame>
#include <QHBoxLayout>
#include <QHash>
#include <QLabel>
#include <QMimeData>
#include <QMouseEvent>
#include <QPushButton>
#include <QScrollArea>
#include <QSpinBox>
#include <QDoubleSpinBox>
#include <QPlainTextEdit>
#include <QVBoxLayout>
#include <vector>
#include <QPainter>
#include <QPaintEvent>
#include <QPolygon>
#include <QResizeEvent>
#include <QTimer>
#include <cmath>
#include <functional>
#include <limits>
#include <algorithm>

namespace {

constexpr char kFxMime[] = "application/x-kp-fxslot-index";

QJsonObject defaultParamsForVideoType(const QString &type)
{
    if (type == QStringLiteral("pan")) {
        QJsonObject o;
        o.insert(QStringLiteral("mode"), QStringLiteral("fit"));
        o.insert(QStringLiteral("panX"), 0.0);
        o.insert(QStringLiteral("panY"), 0.0);
        o.insert(QStringLiteral("zoom"), 1.0);
        return o;
    }
    if (type == QStringLiteral("mirror")) {
        QJsonObject o;
        o.insert(QStringLiteral("horizontal"), true);
        o.insert(QStringLiteral("vertical"), false);
        return o;
    }
    if (type == QStringLiteral("blur")) {
        QJsonObject o;
        o.insert(QStringLiteral("radius"), 8);
        return o;
    }
    if (type == QStringLiteral("greenscreen")) {
        QJsonObject o;
        o.insert(QStringLiteral("keyR"), 0);
        o.insert(QStringLiteral("keyG"), 255);
        o.insert(QStringLiteral("keyB"), 0);
        o.insert(QStringLiteral("tolerance"), 70);
        o.insert(QStringLiteral("softness"), 25);
        return o;
    }
    if (type == QStringLiteral("edges")) {
        QJsonObject o;
        o.insert(QStringLiteral("threshold"), 48);
        return o;
    }
    if (type == QStringLiteral("noise")) {
        QJsonObject o;
        o.insert(QStringLiteral("amount"), 0.08);
        return o;
    }
    if (type == QStringLiteral("vhs")) {
        QJsonObject o;
        o.insert(QStringLiteral("rgbShift"), 4);
        o.insert(QStringLiteral("bleed"), 0.4);
        return o;
    }
    if (type == QStringLiteral("glow")) {
        QJsonObject o;
        o.insert(QStringLiteral("strength"), 0.65);
        return o;
    }
    if (type == QStringLiteral("eventgrid")) {
        QJsonObject o;
        o.insert(QStringLiteral("gridCols"), 1);
        o.insert(QStringLiteral("gridRows"), 1);
        o.insert(QStringLiteral("centerGrid"), true);
        return o;
    }
    if (type == QStringLiteral("cammotion")) {
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
    return QJsonObject();
}

QJsonObject defaultParamsForAudioType(const QString &type)
{
    if (type == QStringLiteral("softclip")) {
        QJsonObject o;
        o.insert(QStringLiteral("drive"), 1.2);
        return o;
    }
    if (type == QStringLiteral("lowpass")) {
        QJsonObject o;
        o.insert(QStringLiteral("cutoffHz"), 6000.0);
        return o;
    }
    if (type == QStringLiteral("audiogrid")) {
        QJsonObject o;
        o.insert(QStringLiteral("gridCols"), 1);
        o.insert(QStringLiteral("gridRows"), 1);
        o.insert(QStringLiteral("spreadMs"), 4.0);
        return o;
    }
    if (type == QStringLiteral("slowedreverb")) {
        QJsonObject o;
        o.insert(QStringLiteral("playbackRate"), 0.82);
        o.insert(QStringLiteral("reverbMix"), 0.38);
        o.insert(QStringLiteral("reverbDelayMs"), 140.0);
        o.insert(QStringLiteral("reverbDecay"), 0.58);
        o.insert(QStringLiteral("reverbDamp"), 0.32);
        o.insert(QStringLiteral("reverbGain"), 0.95);
        o.insert(QStringLiteral("lockTimelineDuration"), false);
        return o;
    }
    return QJsonObject();
}

QString humanTypeName(const QString &id)
{
    static const QHash<QString, QString> map{{QStringLiteral("pan"), QStringLiteral("Pan")},
                                             {QStringLiteral("cammotion"), QStringLiteral("Camera motion")},
                                             {QStringLiteral("mirror"), QStringLiteral("Mirror")},
                                             {QStringLiteral("blur"), QStringLiteral("Blur")},
                                             {QStringLiteral("greenscreen"), QStringLiteral("Chroma")},
                                             {QStringLiteral("edges"), QStringLiteral("Edges")},
                                             {QStringLiteral("noise"), QStringLiteral("Noise")},
                                             {QStringLiteral("vhs"), QStringLiteral("VHS")},
                                             {QStringLiteral("glow"), QStringLiteral("Glow")},
                                             {QStringLiteral("eventgrid"), QStringLiteral("Event grid")},
                                             {QStringLiteral("audiogrid"), QStringLiteral("Audio grid")},
                                             {QStringLiteral("softclip"), QStringLiteral("Soft clip")},
                                             {QStringLiteral("lowpass"), QStringLiteral("Low-pass")},
                                             {QStringLiteral("slowedreverb"), QStringLiteral("Slowed + reverb")}};
    return map.value(id, id.isEmpty() ? QStringLiteral("?") : id);
}

class FxChainCard : public QFrame
{
    Q_OBJECT
public:
    explicit FxChainCard(int chainIndex, const QString &typeId, QWidget *parent = nullptr)
        : QFrame(parent)
        , index(chainIndex)
        , typeId(typeId)
    {
        setFrameShape(QFrame::StyledPanel);
        setLineWidth(1);
        setMidLineWidth(0);
        setMinimumSize(92, 76);
        setMaximumWidth(120);
        setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Minimum);
        setAcceptDrops(true);
        auto *v = new QVBoxLayout(this);
        v->setContentsMargins(8, 8, 8, 8);
        auto *t = new QLabel(humanTypeName(typeId), this);
        t->setWordWrap(true);
        t->setAlignment(Qt::AlignCenter);
        QFont f = t->font();
        f.setBold(true);
        f.setPointSizeF(std::max(8.0, f.pointSizeF()));
        t->setFont(f);
        t->setAttribute(Qt::WA_TransparentForMouseEvents, true);
        v->addWidget(t);
        auto *sub = new QLabel(typeId, this);
        sub->setAlignment(Qt::AlignCenter);
        sub->setStyleSheet(QStringLiteral("color:#9aa4b2;font-size:10px;"));
        sub->setAttribute(Qt::WA_TransparentForMouseEvents, true);
        v->addWidget(sub);
        setCursor(Qt::PointingHandCursor);
    }

    int chainIndex() const { return index; }

    void setChainSelected(bool sel)
    {
        selected = sel;
        setStyleSheet(sel ? QStringLiteral("QFrame { background:#2a3544; border:2px solid #5b8fd4; border-radius:6px; }")
                         : QStringLiteral("QFrame { background:#22262c; border:1px solid #3d4652; border-radius:6px; }"));
    }

signals:
    void clicked(int index);
    void reorderRequested(int fromIndex, int toIndex);

protected:
    void mousePressEvent(QMouseEvent *e) override
    {
        if (e->button() == Qt::LeftButton) {
            pressPos = e->pos();
            dragStarted = false;
        }
        QFrame::mousePressEvent(e);
    }

    void mouseMoveEvent(QMouseEvent *e) override
    {
        if (!(e->buttons() & Qt::LeftButton) || dragStarted) {
            QFrame::mouseMoveEvent(e);
            return;
        }
        if ((e->pos() - pressPos).manhattanLength() < QApplication::startDragDistance()) {
            QFrame::mouseMoveEvent(e);
            return;
        }
        dragStarted = true;
        auto *mime = new QMimeData;
        mime->setData(QString::fromLatin1(kFxMime), QByteArray::number(index));
        QDrag drag(this);
        drag.setMimeData(mime);
        (void)drag.exec(Qt::MoveAction);
    }

    void mouseReleaseEvent(QMouseEvent *e) override
    {
        if (e->button() == Qt::LeftButton && !dragStarted) {
            emit clicked(index);
        }
        dragStarted = false;
        QFrame::mouseReleaseEvent(e);
    }

    void dragEnterEvent(QDragEnterEvent *e) override
    {
        if (e->mimeData()->hasFormat(QString::fromLatin1(kFxMime))) {
            e->acceptProposedAction();
        } else {
            e->ignore();
        }
    }

    void dragMoveEvent(QDragMoveEvent *e) override
    {
        if (e->mimeData()->hasFormat(QString::fromLatin1(kFxMime))) {
            e->acceptProposedAction();
        } else {
            e->ignore();
        }
    }

    void dropEvent(QDropEvent *e) override
    {
        const QByteArray raw = e->mimeData()->data(QString::fromLatin1(kFxMime));
        bool ok = false;
        const int from = raw.trimmed().toInt(&ok);
        if (!ok || from < 0) {
            e->ignore();
            return;
        }
        if (from != index) {
            emit reorderRequested(from, index);
        }
        e->acceptProposedAction();
    }

private:
    int index = 0;
    QString typeId;
    QPoint pressPos;
    bool dragStarted = false;
    bool selected = false;
};

} // namespace

constexpr char kFxAutomationMime[] = "application/x-kp-fx-automation";

/// Multi-row envelopes (one lane per parameter). Time axis auto-fits all keyframe t values; green dashed lines mark clip 0 and 1.
/// Keyframe t may be outside [0,1] for edits after trimming. Playback still samples automation at clip-normalized u ∈ [0,1].
class FxAutomationMultiLaneWidget : public QWidget
{
public:
    explicit FxAutomationMultiLaneWidget(QWidget *parent = nullptr)
        : QWidget(parent)
    {
        setMinimumHeight(96);
        setMinimumWidth(320);
        setMouseTracking(true);
    }

    void setParamKeys(const QStringList &keys, const QStringList &labels)
    {
        paramKeys = keys;
        paramLabels = labels.size() == keys.size() ? labels : keys;
        setMinimumHeight(28 + std::max(1, paramKeys.size()) * 22 + 24);
        recomputeBounds();
        updateGeometry();
        update();
    }

    void setKeyframes(QJsonArray k)
    {
        keyframesData = std::move(k);
        sortKeyframesStable();
        recomputeBounds();
        update();
    }

    QJsonArray keyframes() const { return keyframesData; }

    int selectedKeyframeIndex() const { return selectedIndex; }

    bool removeSelectedKeyframe()
    {
        if (selectedIndex < 0 || keyframesData.size() <= 1) {
            return false;
        }
        keyframesData.removeAt(selectedIndex);
        selectedIndex = std::min(selectedIndex, keyframesData.size() - 1);
        sortKeyframesStable();
        recomputeBounds();
        update();
        return true;
    }

    void refreshTimeScale()
    {
        recomputeBounds();
        update();
    }

    std::function<void()> onEdited;
    std::function<void(int)> onKeyframeSelected;

protected:
    void resizeEvent(QResizeEvent *e) override
    {
        recomputeBounds();
        QWidget::resizeEvent(e);
    }

    void paintEvent(QPaintEvent *) override
    {
        QPainter painter(this);
        painter.fillRect(rect(), QColor(22, 24, 28));

        const int leftMargin = 112;
        const int laneH = 22;
        const int topPad = 22;
        const int lanesH = std::max(1, paramKeys.size()) * laneH;
        trackRect = QRect(leftMargin, topPad, std::max(40, width() - leftMargin - 10), lanesH);

        painter.setPen(QColor(140, 150, 165));
        painter.drawText(8, 16,
                         tr("Envelope time (t = position on clip; dashed lines = clip in/out)."));

        for (double mark : {0.0, 1.0}) {
            if (mark + 1e-9 >= tLo && mark <= tHi + 1e-9) {
                const int x = timeToX(mark);
                painter.setPen(QPen(QColor(55, 160, 95, 200), 1, Qt::DashLine));
                painter.drawLine(x, topPad - 2, x, topPad + lanesH + 2);
            }
        }

        painter.setPen(QPen(QColor(65, 74, 88), 1));
        painter.drawRect(trackRect.adjusted(0, 0, -1, -1));

        int y = trackRect.top();
        for (int li = 0; li < paramKeys.size(); ++li) {
            const QString &pk = paramKeys[li];
            const QString lab = li < paramLabels.size() ? paramLabels[li] : pk;
            painter.setPen(QColor(185, 193, 206));
            painter.drawText(6, y + 15, lab);
            const QRect laneRect(trackRect.left(), y, trackRect.width(), laneH);
            painter.setPen(QPen(QColor(48, 54, 62), 1));
            painter.drawLine(laneRect.left(), laneRect.center().y(), laneRect.right(), laneRect.center().y());

            for (int i = 0; i < keyframesData.size(); ++i) {
                const QJsonObject o = keyframesData.at(i).toObject();
                if (!o.contains(pk)) {
                    continue;
                }
                const double t = o.value(QStringLiteral("t")).toDouble();
                const int cx = timeToX(t);
                const int cy = laneRect.center().y();
                const QColor fill = (i == selectedIndex) ? QColor(91, 143, 212) : QColor(120, 133, 152);
                painter.setBrush(fill);
                painter.setPen(QPen(QColor(235, 240, 250), 1));
                QPolygon dia;
                dia << QPoint(cx, cy - 5) << QPoint(cx + 5, cy) << QPoint(cx, cy + 5) << QPoint(cx - 5, cy);
                painter.drawPolygon(dia);
            }
            y += laneH;
        }
    }

    void mousePressEvent(QMouseEvent *e) override
    {
        const int idx = hitTest(e->pos());
        if (idx >= 0) {
            selectedIndex = idx;
            update();
            if (onKeyframeSelected) {
                onKeyframeSelected(idx);
            }
            dragging = true;
            dragStartX = e->pos().x();
            const QJsonObject o = keyframesData.at(idx).toObject();
            dragStartT = o.value(QStringLiteral("t")).toDouble();
            return;
        }
        selectedIndex = -1;
        update();
        if (onKeyframeSelected) {
            onKeyframeSelected(-1);
        }
        QWidget::mousePressEvent(e);
    }

    void mouseMoveEvent(QMouseEvent *e) override
    {
        if (dragging && selectedIndex >= 0 && selectedIndex < keyframesData.size()) {
            const double dt = xToTimeDelta(e->pos().x() - dragStartX);
            QJsonObject o = keyframesData.at(selectedIndex).toObject();
            o.insert(QStringLiteral("t"), dragStartT + dt);
            keyframesData[selectedIndex] = o;
            recomputeBounds();
            update();
            return;
        }
        QWidget::mouseMoveEvent(e);
    }

    void mouseReleaseEvent(QMouseEvent *e) override
    {
        if (dragging) {
            dragging = false;
            sortKeyframesStable();
            recomputeBounds();
            update();
            if (onEdited) {
                onEdited();
            }
            return;
        }
        QWidget::mouseReleaseEvent(e);
    }

    void mouseDoubleClickEvent(QMouseEvent *e) override
    {
        const int idx = hitTest(e->pos());
        if (idx >= 0 && keyframesData.size() > 1) {
            keyframesData.removeAt(idx);
            selectedIndex = std::min(selectedIndex, keyframesData.size() - 1);
            sortKeyframesStable();
            recomputeBounds();
            update();
            if (onKeyframeSelected) {
                onKeyframeSelected(selectedIndex);
            }
            if (onEdited) {
                onEdited();
            }
            return;
        }
        QWidget::mouseDoubleClickEvent(e);
    }

private:
    void sortKeyframesStable()
    {
        std::vector<std::pair<double, QJsonValue>> items;
        items.reserve(static_cast<size_t>(keyframesData.size()));
        for (const QJsonValue &jv : keyframesData) {
            if (!jv.isObject()) {
                continue;
            }
            const QJsonObject o = jv.toObject();
            items.push_back({o.value(QStringLiteral("t")).toDouble(), jv});
        }
        std::stable_sort(items.begin(), items.end(), [](const auto &a, const auto &b) { return a.first < b.first; });
        keyframesData = QJsonArray();
        for (const auto &pr : items) {
            keyframesData.append(pr.second);
        }
    }

    void recomputeBounds()
    {
        double mn = std::numeric_limits<double>::infinity();
        double mx = -std::numeric_limits<double>::infinity();
        for (const QJsonValue &jv : keyframesData) {
            if (!jv.isObject()) {
                continue;
            }
            const double t = jv.toObject().value(QStringLiteral("t")).toDouble();
            mn = std::min(mn, t);
            mx = std::max(mx, t);
        }
        if (!std::isfinite(mn) || !std::isfinite(mx)) {
            tLo = -0.2;
            tHi = 1.2;
            return;
        }
        double pad = 0.12 * std::max(1e-6, mx - mn);
        if (pad < 0.06) {
            pad = 0.06;
        }
        tLo = std::min(0.0, mn) - pad;
        tHi = std::max(1.0, mx) + pad;
        if (tHi - tLo < 0.3) {
            tLo -= 0.15;
            tHi += 0.15;
        }
    }

    int timeToX(double t) const
    {
        double u = (t - tLo) / (tHi - tLo);
        u = std::clamp(u, 0.0, 1.0);
        return trackRect.left() + static_cast<int>(std::lround(u * std::max(1, trackRect.width() - 1)));
    }

    double xToTimeDelta(int dx) const
    {
        return static_cast<double>(dx) / static_cast<double>(std::max(1, trackRect.width() - 1)) * (tHi - tLo);
    }

    int hitTest(QPoint pos) const
    {
        if (!trackRect.contains(pos)) {
            return -1;
        }
        constexpr int laneH = 22;
        const int li = (pos.y() - trackRect.top()) / laneH;
        if (li < 0 || li >= paramKeys.size()) {
            return -1;
        }
        const QString &pk = paramKeys[li];
        const int yLane = trackRect.top() + li * laneH;
        const int cy = yLane + laneH / 2;
        for (int i = 0; i < keyframesData.size(); ++i) {
            const QJsonObject o = keyframesData.at(i).toObject();
            if (!o.contains(pk)) {
                continue;
            }
            const double t = o.value(QStringLiteral("t")).toDouble();
            const int cx = timeToX(t);
            if (std::abs(pos.x() - cx) <= 8 && std::abs(pos.y() - cy) <= 9) {
                return i;
            }
        }
        return -1;
    }

    QJsonArray keyframesData;
    QStringList paramKeys;
    QStringList paramLabels;
    double tLo = -0.2;
    double tHi = 1.2;
    QRect trackRect;
    int selectedIndex = -1;
    bool dragging = false;
    int dragStartX = 0;
    double dragStartT = 0.0;
};

static QString automationLabelForKey(const QString &key)
{
    static const QHash<QString, QString> map{{QStringLiteral("mode"), QStringLiteral("Framing")},
                                             {QStringLiteral("panX"), QStringLiteral("Pan X")},
                                             {QStringLiteral("panY"), QStringLiteral("Pan Y")},
                                             {QStringLiteral("zoom"), QStringLiteral("Zoom")},
                                             {QStringLiteral("ease"), QStringLiteral("Ease")},
                                             {QStringLiteral("timeScale"), QStringLiteral("Time scale")},
                                             {QStringLiteral("horizontal"), QStringLiteral("Flip H")},
                                             {QStringLiteral("vertical"), QStringLiteral("Flip V")},
                                             {QStringLiteral("radius"), QStringLiteral("Radius")},
                                             {QStringLiteral("keyR"), QStringLiteral("Key R")},
                                             {QStringLiteral("keyG"), QStringLiteral("Key G")},
                                             {QStringLiteral("keyB"), QStringLiteral("Key B")},
                                             {QStringLiteral("tolerance"), QStringLiteral("Tolerance")},
                                             {QStringLiteral("softness"), QStringLiteral("Softness")},
                                             {QStringLiteral("threshold"), QStringLiteral("Threshold")},
                                             {QStringLiteral("amount"), QStringLiteral("Amount")},
                                             {QStringLiteral("rgbShift"), QStringLiteral("RGB shift")},
                                             {QStringLiteral("bleed"), QStringLiteral("Bleed")},
                                             {QStringLiteral("strength"), QStringLiteral("Strength")},
                                             {QStringLiteral("gridCols"), QStringLiteral("Grid cols")},
                                             {QStringLiteral("gridRows"), QStringLiteral("Grid rows")},
                                             {QStringLiteral("centerGrid"), QStringLiteral("Center grid")},
                                             {QStringLiteral("drive"), QStringLiteral("Drive")},
                                             {QStringLiteral("cutoffHz"), QStringLiteral("Cutoff")},
                                             {QStringLiteral("spreadMs"), QStringLiteral("Spread ms")},
                                             {QStringLiteral("playbackRate"), QStringLiteral("Playback rate")},
                                             {QStringLiteral("reverbMix"), QStringLiteral("Reverb mix")}};
    return map.value(key, key);
}

static FxAutomationMultiLaneWidget *findAutomationMultiLane(QWidget *paramPanel)
{
    return dynamic_cast<FxAutomationMultiLaneWidget *>(
        paramPanel->findChild<QWidget *>(QStringLiteral("fx_param_automation_multilane")));
}

static QStringList animatableParamKeys(const QString &type)
{
    if (type == QStringLiteral("pan")) {
        return {QStringLiteral("mode"), QStringLiteral("panX"), QStringLiteral("panY"), QStringLiteral("zoom")};
    }
    if (type == QStringLiteral("cammotion")) {
        return {QStringLiteral("mode"), QStringLiteral("ease"), QStringLiteral("timeScale")};
    }
    if (type == QStringLiteral("mirror")) {
        return {QStringLiteral("horizontal"), QStringLiteral("vertical")};
    }
    if (type == QStringLiteral("blur")) {
        return {QStringLiteral("radius")};
    }
    if (type == QStringLiteral("greenscreen")) {
        return {QStringLiteral("keyR"), QStringLiteral("keyG"), QStringLiteral("keyB"),
                QStringLiteral("tolerance"), QStringLiteral("softness")};
    }
    if (type == QStringLiteral("edges")) {
        return {QStringLiteral("threshold")};
    }
    if (type == QStringLiteral("noise")) {
        return {QStringLiteral("amount")};
    }
    if (type == QStringLiteral("vhs")) {
        return {QStringLiteral("rgbShift"), QStringLiteral("bleed")};
    }
    if (type == QStringLiteral("glow")) {
        return {QStringLiteral("strength")};
    }
    if (type == QStringLiteral("eventgrid")) {
        return {QStringLiteral("gridCols"), QStringLiteral("gridRows"), QStringLiteral("centerGrid")};
    }
    if (type == QStringLiteral("audiogrid")) {
        return {QStringLiteral("gridCols"), QStringLiteral("gridRows"), QStringLiteral("spreadMs")};
    }
    if (type == QStringLiteral("softclip")) {
        return {QStringLiteral("drive")};
    }
    if (type == QStringLiteral("lowpass")) {
        return {QStringLiteral("cutoffHz")};
    }
    if (type == QStringLiteral("slowedreverb")) {
        return {QStringLiteral("playbackRate"), QStringLiteral("reverbMix")};
    }
    return {};
}

namespace {

/// Remove all layout items then delete the layout. Uses count() so we never call takeAt(0) on an empty
/// QFormLayout (that emits "QFormLayout::takeAt: Invalid index 0" and can corrupt state).
static void deleteLayoutAndChildren(QWidget *w)
{
    if (!w) {
        return;
    }
    QLayout *layout = w->layout();
    if (!layout) {
        return;
    }
    while (layout->count() > 0) {
        QLayoutItem *it = layout->takeAt(0);
        if (!it) {
            break;
        }
        if (QWidget *cw = it->widget()) {
            delete cw;
        }
        delete it;
    }
    delete layout;
}

} // namespace

FxChainEditorWidget::FxChainEditorWidget(QWidget *parent)
    : QWidget(parent)
{
    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(0, 0, 0, 0);
    root->setSpacing(8);

    auto *chainLabel =
        new QLabel(tr("Effect chain (decoder → output). Drag a tile onto another to reorder."), this);
    chainLabel->setWordWrap(true);
    root->addWidget(chainLabel);

    chainScroll = new QScrollArea(this);
    chainScroll->setWidgetResizable(true);
    chainScroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    chainScroll->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    chainScroll->setMinimumHeight(100);
    chainScroll->setMaximumHeight(108);
    chainInner = new QWidget(chainScroll);
    chainScroll->setWidget(chainInner);
    root->addWidget(chainScroll);

    auto *paramLabel = new QLabel(tr("Parameters"), this);
    root->addWidget(paramLabel);
    paramPanel = new QWidget(this);
    paramPanel->setMinimumHeight(140);
    root->addWidget(paramPanel, 1);

    auto *addRow = new QHBoxLayout();
    addCombo = new QComboBox(this);
    auto *addBtn = new QPushButton(tr("Add"), this);
    auto *removeBtn = new QPushButton(tr("Remove"), this);
    addRow->addWidget(addCombo, 1);
    addRow->addWidget(addBtn);
    addRow->addWidget(removeBtn);
    root->addLayout(addRow);

    connect(addBtn, &QPushButton::clicked, this, &FxChainEditorWidget::onAddClicked);
    connect(removeBtn, &QPushButton::clicked, this, &FxChainEditorWidget::onRemoveClicked);

    setMediaKind(MediaKind::Video);
}

void FxChainEditorWidget::setCamMotionEstimateContext(TimelineEditor *timeline, int clipId)
{
    camMotionTimeline = timeline;
    camMotionClipId = clipId;
}

void FxChainEditorWidget::setMediaKind(MediaKind k)
{
    mediaKind = k;
    addCombo->clear();
    if (mediaKind == MediaKind::Video) {
        const struct {
            const char *id;
            const char *label;
        } items[] = {
            {"pan", "Pan / frame"},
            {"cammotion", "Camera motion (keyframes)"},
            {"mirror", "Mirror"},
            {"blur", "Blur"},
            {"greenscreen", "Greenscreen"},
            {"edges", "Edges"},
            {"noise", "Noise"},
            {"vhs", "VHS"},
            {"glow", "Glow"},
            {"eventgrid", "Event grid (tile dup)"},
        };
        for (const auto &it : items) {
            addCombo->addItem(QString::fromUtf8(it.label), QString::fromUtf8(it.id));
        }
    } else {
        addCombo->addItem(tr("Audio grid (multi-tap)"), QStringLiteral("audiogrid"));
        addCombo->addItem(tr("Soft clip"), QStringLiteral("softclip"));
        addCombo->addItem(tr("Low-pass"), QStringLiteral("lowpass"));
        addCombo->addItem(tr("Slowed + reverb"), QStringLiteral("slowedreverb"));
    }
}

void FxChainEditorWidget::setChain(const QJsonArray &chain)
{
    workingChain = chain;
    if (workingChain.isEmpty()) {
        selectedFxRow = -1;
    } else if (selectedFxRow < 0 || selectedFxRow >= workingChain.size()) {
        selectedFxRow = workingChain.size() - 1;
    }
    rebuildChainStrip();
    rebuildParamPanel();
}

void FxChainEditorWidget::selectRow(int row)
{
    if (workingChain.isEmpty() || row < 0 || row >= workingChain.size()) {
        selectedFxRow = -1;
    } else {
        selectedFxRow = row;
    }
    rebuildChainStrip();
    rebuildParamPanel();
}

QString FxChainEditorWidget::effectTypeAt(int row) const
{
    if (row < 0 || row >= workingChain.size()) {
        return {};
    }
    return workingChain.at(row).toObject().value(QStringLiteral("type")).toString();
}

void FxChainEditorWidget::onChainCardClicked(int index)
{
    selectedFxRow = index;
    for (int i = 0; i < chainLayout->count(); ++i) {
        QLayoutItem *it = chainLayout->itemAt(i);
        if (!it || !it->widget()) {
            continue;
        }
        auto *card = qobject_cast<FxChainCard *>(it->widget());
        if (card) {
            card->setChainSelected(card->chainIndex() == index);
        }
    }
    rebuildParamPanel();
}

void FxChainEditorWidget::onChainReorder(int fromIndex, int toIndex)
{
    QTimer::singleShot(0, this, [this, fromIndex, toIndex]() {
        if (fromIndex < 0 || fromIndex >= workingChain.size() || toIndex < 0 || toIndex >= workingChain.size() ||
            fromIndex == toIndex) {
            return;
        }
        applyParamsFromPanelToChain();
        const QJsonValue v = workingChain.takeAt(fromIndex);
        int adj = toIndex;
        if (fromIndex < toIndex) {
            adj = toIndex - 1;
        }
        workingChain.insert(adj, v);
        selectedFxRow = adj;
        rebuildChainStrip();
        rebuildParamPanel();
        emit chainChanged(QJsonArray(workingChain));
    });
}

void FxChainEditorWidget::rebuildChainStrip()
{
    if (QLayout *old = chainInner->layout()) {
        QLayoutItem *it = nullptr;
        while ((it = old->takeAt(0)) != nullptr) {
            delete it->widget();
            delete it;
        }
        delete old;
    }
    chainLayout = new QHBoxLayout(chainInner);
    chainLayout->setContentsMargins(8, 8, 8, 8);
    chainLayout->setSpacing(4);

    auto *src = new QLabel(tr("Media"), chainInner);
    src->setAlignment(Qt::AlignCenter);
    src->setMinimumSize(64, 72);
    src->setMaximumHeight(76);
    src->setStyleSheet(
        QStringLiteral("QLabel { background:#1a1d22; border:1px dashed #555; border-radius:6px; color:#8b95a5; }"));
    chainLayout->addWidget(src);

    for (int i = 0; i < workingChain.size(); ++i) {
        auto *arrow = new QLabel(QStringLiteral("→"), chainInner);
        arrow->setStyleSheet(QStringLiteral("color:#6b7788;font-size:18px;padding:0 2px;"));
        arrow->setAlignment(Qt::AlignVCenter);
        chainLayout->addWidget(arrow);

        const QString tid = effectTypeAt(i);
        auto *card = new FxChainCard(i, tid, chainInner);
        card->setChainSelected(i == selectedFxRow);
        connect(card, &FxChainCard::clicked, this, &FxChainEditorWidget::onChainCardClicked);
        connect(card, &FxChainCard::reorderRequested, this, &FxChainEditorWidget::onChainReorder);
        chainLayout->addWidget(card);
    }
    chainLayout->addStretch(1);
}

void FxChainEditorWidget::applyParamsFromPanelToChain()
{
    if (selectedFxRow < 0 || selectedFxRow >= workingChain.size() || !paramPanel) {
        return;
    }
    const QJsonValue rowVal = workingChain.at(selectedFxRow);
    if (!rowVal.isObject()) {
        return;
    }
    QJsonObject slot = rowVal.toObject();
    QString type = slot.value(QStringLiteral("type")).toString();
    if (type.isEmpty()) {
        type = effectTypeAt(selectedFxRow);
        if (type.isEmpty()) {
            return;
        }
        slot.insert(QStringLiteral("type"), type);
    }
    QJsonObject p = slot.value(QStringLiteral("params")).toObject();

    if (type == QStringLiteral("pan")) {
        if (auto *w = paramPanel->findChild<QComboBox *>(QStringLiteral("fx_pan_mode"))) {
            p.insert(QStringLiteral("mode"), w->currentData().toString());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_pan_panX"))) {
            p.insert(QStringLiteral("panX"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_pan_panY"))) {
            p.insert(QStringLiteral("panY"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_pan_zoom"))) {
            p.insert(QStringLiteral("zoom"), w->value());
        }
    } else if (type == QStringLiteral("cammotion")) {
        if (auto *w = paramPanel->findChild<QComboBox *>(QStringLiteral("fx_cammotion_mode"))) {
            p.insert(QStringLiteral("mode"), w->currentData().toString());
        }
        if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_cammotion_ease"))) {
            p.insert(QStringLiteral("ease"), w->isChecked());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_cammotion_timescale"))) {
            p.insert(QStringLiteral("timeScale"), w->value());
        }
        if (auto *w = paramPanel->findChild<QPlainTextEdit *>(QStringLiteral("fx_cammotion_keyframes_json"))) {
            const QString raw = w->toPlainText().trimmed();
            QJsonParseError err{};
            const QJsonDocument doc = QJsonDocument::fromJson(raw.toUtf8(), &err);
            QJsonArray kf;
            if (doc.isArray()) {
                kf = doc.array();
            } else if (doc.isObject()) {
                kf = doc.object().value(QStringLiteral("keyframes")).toArray();
            }
            p.insert(QStringLiteral("keyframes"), kf);
        }
    } else if (type == QStringLiteral("mirror")) {
        if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_mirror_h"))) {
            p.insert(QStringLiteral("horizontal"), w->isChecked());
        }
        if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_mirror_v"))) {
            p.insert(QStringLiteral("vertical"), w->isChecked());
        }
    } else if (type == QStringLiteral("blur")) {
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_blur_radius"))) {
            p.insert(QStringLiteral("radius"), w->value());
        }
    } else if (type == QStringLiteral("greenscreen")) {
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_chroma_tol"))) {
            p.insert(QStringLiteral("tolerance"), w->value());
        }
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_chroma_soft"))) {
            p.insert(QStringLiteral("softness"), w->value());
        }
        if (auto *w = paramPanel->findChild<QFrame *>(QStringLiteral("fx_chroma_swatch"))) {
            const QColor c = w->property("keyColor").value<QColor>();
            p.insert(QStringLiteral("keyR"), c.red());
            p.insert(QStringLiteral("keyG"), c.green());
            p.insert(QStringLiteral("keyB"), c.blue());
        }
    } else if (type == QStringLiteral("edges")) {
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_edges_th"))) {
            p.insert(QStringLiteral("threshold"), w->value());
        }
    } else if (type == QStringLiteral("noise")) {
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_noise_amt"))) {
            p.insert(QStringLiteral("amount"), w->value());
        }
    } else if (type == QStringLiteral("vhs")) {
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_vhs_shift"))) {
            p.insert(QStringLiteral("rgbShift"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_vhs_bleed"))) {
            p.insert(QStringLiteral("bleed"), w->value());
        }
    } else if (type == QStringLiteral("glow")) {
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_glow_str"))) {
            p.insert(QStringLiteral("strength"), w->value());
        }
    } else if (type == QStringLiteral("softclip")) {
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sc_drive"))) {
            p.insert(QStringLiteral("drive"), w->value());
        }
    } else if (type == QStringLiteral("lowpass")) {
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_lp_cut"))) {
            p.insert(QStringLiteral("cutoffHz"), w->value());
        }
    } else if (type == QStringLiteral("eventgrid")) {
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_grid_cols"))) {
            p.insert(QStringLiteral("gridCols"), w->value());
        }
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_grid_rows"))) {
            p.insert(QStringLiteral("gridRows"), w->value());
        }
        if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_grid_center"))) {
            p.insert(QStringLiteral("centerGrid"), w->isChecked());
        }
    } else if (type == QStringLiteral("audiogrid")) {
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_audio_grid_cols"))) {
            p.insert(QStringLiteral("gridCols"), w->value());
        }
        if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_audio_grid_rows"))) {
            p.insert(QStringLiteral("gridRows"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_audio_spread_ms"))) {
            p.insert(QStringLiteral("spreadMs"), w->value());
        }
    } else if (type == QStringLiteral("slowedreverb")) {
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_rate"))) {
            p.insert(QStringLiteral("playbackRate"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_mix"))) {
            p.insert(QStringLiteral("reverbMix"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_delay"))) {
            p.insert(QStringLiteral("reverbDelayMs"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_decay"))) {
            p.insert(QStringLiteral("reverbDecay"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_damp"))) {
            p.insert(QStringLiteral("reverbDamp"), w->value());
        }
        if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_gain"))) {
            p.insert(QStringLiteral("reverbGain"), w->value());
        }
        if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_sr_lock_dur"))) {
            p.insert(QStringLiteral("lockTimelineDuration"), w->isChecked());
        }
    }

    if (FxAutomationMultiLaneWidget *ml = findAutomationMultiLane(paramPanel)) {
        QJsonArray arr = ml->keyframes();
        if (fxAutomationSelectedKeyframe >= 0 && fxAutomationSelectedKeyframe < arr.size()) {
            QJsonObject kobj = arr.at(fxAutomationSelectedKeyframe).toObject();
            const QStringList ak = animatableParamKeys(type);
            for (const QString &k : ak) {
                if (p.contains(k)) {
                    kobj.insert(k, p.value(k));
                }
            }
            arr[fxAutomationSelectedKeyframe] = kobj;
        }
        p.insert(QStringLiteral("automation"), arr);
    }

    slot.insert(QStringLiteral("params"), p);
    workingChain[selectedFxRow] = slot;
}

void FxChainEditorWidget::rebuildParamPanel()
{
    suppressParamRefresh = 1;
    fxAutomationSelectedKeyframe = -1;
    deleteLayoutAndChildren(paramPanel);

    if (selectedFxRow < 0 || selectedFxRow >= workingChain.size()) {
        auto *lay = new QVBoxLayout(paramPanel);
        lay->addWidget(new QLabel(tr("Select an effect in the chain."), paramPanel));
        suppressParamRefresh = 0;
        return;
    }

    const QJsonObject slot = workingChain.at(selectedFxRow).toObject();
    const QString type = slot.value(QStringLiteral("type")).toString();
    QJsonObject p = slot.value(QStringLiteral("params")).toObject();
    auto *form = new QFormLayout(paramPanel);
    form->setContentsMargins(12, 8, 12, 8);
    form->setSpacing(10);

    auto hook = [this]() {
        if (suppressParamRefresh) {
            return;
        }
        applyParamsFromPanelToChain();
        emit chainChanged(QJsonArray(workingChain));
    };

    if (type == QStringLiteral("pan")) {
        auto *mode = new QComboBox(paramPanel);
        mode->setObjectName(QStringLiteral("fx_pan_mode"));
        mode->addItem(tr("Fit (letterbox)"), QStringLiteral("fit"));
        mode->addItem(tr("Fill (crop)"), QStringLiteral("fill"));
        mode->addItem(tr("Stretch"), QStringLiteral("stretch"));
        const QString m = p.value(QStringLiteral("mode")).toString(QStringLiteral("fit"));
        const int mi = std::max(0, mode->findData(m));
        mode->setCurrentIndex(mi >= 0 ? mi : 0);
        form->addRow(tr("Framing"), mode);
        connect(mode, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [hook](int) { hook(); });

        auto *panX = new QDoubleSpinBox(paramPanel);
        panX->setObjectName(QStringLiteral("fx_pan_panX"));
        panX->setRange(-1.0, 1.0);
        panX->setSingleStep(0.02);
        panX->setDecimals(3);
        panX->setValue(p.value(QStringLiteral("panX")).toDouble(0.0));
        form->addRow(tr("Pan X"), panX);
        connect(panX, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });

        auto *panY = new QDoubleSpinBox(paramPanel);
        panY->setObjectName(QStringLiteral("fx_pan_panY"));
        panY->setRange(-1.0, 1.0);
        panY->setSingleStep(0.02);
        panY->setDecimals(3);
        panY->setValue(p.value(QStringLiteral("panY")).toDouble(0.0));
        form->addRow(tr("Pan Y"), panY);
        connect(panY, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });

        auto *zoom = new QDoubleSpinBox(paramPanel);
        zoom->setObjectName(QStringLiteral("fx_pan_zoom"));
        zoom->setRange(0.05, 16.0);
        zoom->setSingleStep(0.05);
        zoom->setDecimals(3);
        zoom->setValue(p.value(QStringLiteral("zoom")).toDouble(1.0));
        form->addRow(tr("Zoom"), zoom);
        connect(zoom, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("cammotion")) {
        auto *mode = new QComboBox(paramPanel);
        mode->setObjectName(QStringLiteral("fx_cammotion_mode"));
        mode->addItem(tr("Fit (letterbox)"), QStringLiteral("fit"));
        mode->addItem(tr("Fill (crop)"), QStringLiteral("fill"));
        mode->addItem(tr("Stretch"), QStringLiteral("stretch"));
        const QString m = p.value(QStringLiteral("mode")).toString(QStringLiteral("fit"));
        const int mi = std::max(0, mode->findData(m));
        mode->setCurrentIndex(mi >= 0 ? mi : 0);
        form->addRow(tr("Framing"), mode);
        connect(mode, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [hook](int) { hook(); });

        auto *ease = new QCheckBox(tr("Smooth segments (smoothstep)"), paramPanel);
        ease->setObjectName(QStringLiteral("fx_cammotion_ease"));
        ease->setChecked(p.value(QStringLiteral("ease")).toBool(true));
        form->addRow(ease);
        connect(ease, &QCheckBox::toggled, this, [hook](bool) { hook(); });

        auto *ts = new QDoubleSpinBox(paramPanel);
        ts->setObjectName(QStringLiteral("fx_cammotion_timescale"));
        ts->setRange(0.05, 8.0);
        ts->setSingleStep(0.05);
        ts->setDecimals(3);
        ts->setValue(p.value(QStringLiteral("timeScale")).toDouble(1.0));
        form->addRow(tr("Time scale (clip progress)"), ts);
        connect(ts, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });

        auto *autoRow = new QWidget(paramPanel);
        auto *autoLay = new QHBoxLayout(autoRow);
        autoLay->setContentsMargins(0, 0, 0, 0);
        auto *samplesSpin = new QSpinBox(autoRow);
        samplesSpin->setObjectName(QStringLiteral("fx_cammotion_samples"));
        samplesSpin->setRange(8, 48);
        samplesSpin->setValue(28);
        samplesSpin->setToolTip(tr("More time samples capture more reversals (slower decode)."));
        auto *targetSpin = new QSpinBox(autoRow);
        targetSpin->setObjectName(QStringLiteral("fx_cammotion_targetkeys"));
        targetSpin->setRange(0, 24);
        targetSpin->setValue(0);
        targetSpin->setSpecialValueText(tr("Auto"));
        targetSpin->setToolTip(tr("Approximate number of keyframes after simplification (0 = auto from samples)."));
        auto *autoBtn = new QPushButton(tr("Auto from clip video…"), autoRow);
        autoBtn->setEnabled(camMotionTimeline != nullptr && camMotionClipId >= 0);
        autoLay->addWidget(new QLabel(tr("Samples:"), autoRow));
        autoLay->addWidget(samplesSpin);
        autoLay->addWidget(new QLabel(tr("Keys:"), autoRow));
        autoLay->addWidget(targetSpin);
        autoLay->addWidget(autoBtn, 1);
        form->addRow(tr("Estimate"), autoRow);
        connect(autoBtn, &QPushButton::clicked, this, [this, samplesSpin, targetSpin, mode, hook]() {
            if (!camMotionTimeline || camMotionClipId < 0) {
                return;
            }
            const QString framing = mode->currentData().toString();
            QApplication::setOverrideCursor(Qt::WaitCursor);
            const QJsonArray kf =
                camMotionTimeline->estimateCamMotionKeyframesFromClip(camMotionClipId,
                                                                      samplesSpin->value(),
                                                                      framing,
                                                                      targetSpin->value());
            QApplication::restoreOverrideCursor();
            if (kf.isEmpty()) {
                QMessageBox::warning(this,
                                     tr("Camera motion"),
                                     tr("Could not decode enough frames from this clip (or it is a still image)."));
                return;
            }
            if (auto *pte = paramPanel->findChild<QPlainTextEdit *>(QStringLiteral("fx_cammotion_keyframes_json"))) {
                pte->setPlainText(QString::fromUtf8(QJsonDocument(kf).toJson(QJsonDocument::Compact)));
            }
            hook();
        });

        auto *hint = new QLabel(
            tr("Keyframes: JSON array of objects {\"t\":0..1,\"panX\":…,\"panY\":…,\"zoom\":…}. "
               "Same curve on any clip length — copy text to another clip’s Camera motion."),
            paramPanel);
        hint->setWordWrap(true);
        form->addRow(hint);

        auto *pte = new QPlainTextEdit(paramPanel);
        pte->setObjectName(QStringLiteral("fx_cammotion_keyframes_json"));
        pte->setMinimumHeight(120);
        {
            QFont f = pte->font();
            f.setFamily(QStringLiteral("monospace"));
            pte->setFont(f);
        }
        const QJsonArray kfArr = p.value(QStringLiteral("keyframes")).toArray();
        pte->setPlainText(QString::fromUtf8(QJsonDocument(kfArr).toJson(QJsonDocument::Compact)));
        form->addRow(tr("Keyframes"), pte);
        connect(pte, &QPlainTextEdit::textChanged, this, [hook]() { hook(); });

        auto *btnRow = new QWidget(paramPanel);
        auto *bh = new QHBoxLayout(btnRow);
        bh->setContentsMargins(0, 0, 0, 0);
        auto *copyBtn = new QPushButton(tr("Copy"), btnRow);
        auto *pasteBtn = new QPushButton(tr("Paste"), btnRow);
        bh->addWidget(copyBtn);
        bh->addWidget(pasteBtn);
        bh->addStretch(1);
        form->addRow(tr("Clipboard"), btnRow);
        connect(copyBtn, &QPushButton::clicked, this, [pte]() {
            QGuiApplication::clipboard()->setText(pte->toPlainText());
        });
        connect(pasteBtn, &QPushButton::clicked, this, [pte, hook]() {
            const QString t = QGuiApplication::clipboard()->text();
            if (!t.isEmpty()) {
                pte->setPlainText(t);
            }
            hook();
        });
    } else if (type == QStringLiteral("mirror")) {
        auto *h = new QCheckBox(tr("Horizontal flip"), paramPanel);
        h->setObjectName(QStringLiteral("fx_mirror_h"));
        h->setChecked(p.value(QStringLiteral("horizontal")).toBool(true));
        form->addRow(h);
        connect(h, &QCheckBox::toggled, this, [hook](bool) { hook(); });
        auto *v = new QCheckBox(tr("Vertical flip"), paramPanel);
        v->setObjectName(QStringLiteral("fx_mirror_v"));
        v->setChecked(p.value(QStringLiteral("vertical")).toBool(false));
        form->addRow(v);
        connect(v, &QCheckBox::toggled, this, [hook](bool) { hook(); });
    } else if (type == QStringLiteral("blur")) {
        auto *r = new QSpinBox(paramPanel);
        r->setObjectName(QStringLiteral("fx_blur_radius"));
        r->setRange(1, 48);
        r->setValue(p.value(QStringLiteral("radius")).toInt(8));
        form->addRow(tr("Radius"), r);
        connect(r, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
    } else if (type == QStringLiteral("greenscreen")) {
        const int kr = p.value(QStringLiteral("keyR")).toInt(0);
        const int kg = p.value(QStringLiteral("keyG")).toInt(255);
        const int kb = p.value(QStringLiteral("keyB")).toInt(0);
        QColor key(kr, kg, kb);
        auto *colorRow = new QWidget(paramPanel);
        auto *hlay = new QHBoxLayout(colorRow);
        hlay->setContentsMargins(0, 0, 0, 0);
        auto *swatch = new QFrame(colorRow);
        swatch->setObjectName(QStringLiteral("fx_chroma_swatch"));
        swatch->setFixedSize(40, 28);
        swatch->setProperty("keyColor", key);
        swatch->setStyleSheet(QStringLiteral("background:%1; border:1px solid #555; border-radius:4px;").arg(key.name()));
        auto *pick = new QPushButton(tr("Key color…"), colorRow);
        hlay->addWidget(swatch);
        hlay->addWidget(pick);
        hlay->addStretch(1);
        form->addRow(tr("Key"), colorRow);
        connect(pick, &QPushButton::clicked, this, [this, swatch, hook]() {
            const QColor c = QColorDialog::getColor(swatch->property("keyColor").value<QColor>(), this, tr("Key color"));
            if (c.isValid()) {
                swatch->setProperty("keyColor", c);
                swatch->setStyleSheet(
                    QStringLiteral("background:%1; border:1px solid #555; border-radius:4px;").arg(c.name()));
                hook();
            }
        });

        auto *tol = new QSpinBox(paramPanel);
        tol->setObjectName(QStringLiteral("fx_chroma_tol"));
        tol->setRange(1, 200);
        tol->setValue(p.value(QStringLiteral("tolerance")).toInt(70));
        form->addRow(tr("Tolerance"), tol);
        connect(tol, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });

        auto *soft = new QSpinBox(paramPanel);
        soft->setObjectName(QStringLiteral("fx_chroma_soft"));
        soft->setRange(0, 100);
        soft->setValue(p.value(QStringLiteral("softness")).toInt(25));
        form->addRow(tr("Softness"), soft);
        connect(soft, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
    } else if (type == QStringLiteral("edges")) {
        auto *th = new QSpinBox(paramPanel);
        th->setObjectName(QStringLiteral("fx_edges_th"));
        th->setRange(1, 255);
        th->setValue(p.value(QStringLiteral("threshold")).toInt(48));
        form->addRow(tr("Threshold"), th);
        connect(th, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
    } else if (type == QStringLiteral("noise")) {
        auto *a = new QDoubleSpinBox(paramPanel);
        a->setObjectName(QStringLiteral("fx_noise_amt"));
        a->setRange(0.0, 1.0);
        a->setDecimals(3);
        a->setSingleStep(0.01);
        a->setValue(p.value(QStringLiteral("amount")).toDouble(0.08));
        form->addRow(tr("Amount"), a);
        connect(a, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("vhs")) {
        auto *sh = new QSpinBox(paramPanel);
        sh->setObjectName(QStringLiteral("fx_vhs_shift"));
        sh->setRange(0, 20);
        sh->setValue(p.value(QStringLiteral("rgbShift")).toInt(4));
        form->addRow(tr("RGB shift"), sh);
        connect(sh, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
        auto *bl = new QDoubleSpinBox(paramPanel);
        bl->setObjectName(QStringLiteral("fx_vhs_bleed"));
        bl->setRange(0.0, 1.0);
        bl->setDecimals(2);
        bl->setSingleStep(0.05);
        bl->setValue(p.value(QStringLiteral("bleed")).toDouble(0.4));
        form->addRow(tr("Bleed"), bl);
        connect(bl, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("glow")) {
        auto *s = new QDoubleSpinBox(paramPanel);
        s->setObjectName(QStringLiteral("fx_glow_str"));
        s->setRange(0.0, 2.5);
        s->setDecimals(2);
        s->setSingleStep(0.05);
        s->setValue(p.value(QStringLiteral("strength")).toDouble(0.65));
        form->addRow(tr("Strength"), s);
        connect(s, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("eventgrid")) {
        auto *cols = new QSpinBox(paramPanel);
        cols->setObjectName(QStringLiteral("fx_grid_cols"));
        cols->setRange(1, 512);
        cols->setValue(p.value(QStringLiteral("gridCols")).toInt(1));
        form->addRow(tr("Grid columns"), cols);
        connect(cols, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
        auto *rows = new QSpinBox(paramPanel);
        rows->setObjectName(QStringLiteral("fx_grid_rows"));
        rows->setRange(1, 512);
        rows->setValue(p.value(QStringLiteral("gridRows")).toInt(1));
        form->addRow(tr("Grid rows"), rows);
        connect(rows, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
        auto *center = new QCheckBox(tr("Center grid in frame (letterbox padding)"), paramPanel);
        center->setObjectName(QStringLiteral("fx_grid_center"));
        center->setChecked(p.value(QStringLiteral("centerGrid")).toBool(true));
        form->addRow(center);
        connect(center, &QCheckBox::toggled, this, [hook](bool) { hook(); });
    } else if (type == QStringLiteral("softclip")) {
        auto *d = new QDoubleSpinBox(paramPanel);
        d->setObjectName(QStringLiteral("fx_sc_drive"));
        d->setRange(0.5, 4.0);
        d->setDecimals(2);
        d->setSingleStep(0.05);
        d->setValue(p.value(QStringLiteral("drive")).toDouble(1.2));
        form->addRow(tr("Drive"), d);
        connect(d, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("lowpass")) {
        auto *c = new QDoubleSpinBox(paramPanel);
        c->setObjectName(QStringLiteral("fx_lp_cut"));
        c->setRange(100.0, 20000.0);
        c->setDecimals(0);
        c->setSingleStep(100.0);
        c->setSuffix(QStringLiteral(" Hz"));
        c->setValue(p.value(QStringLiteral("cutoffHz")).toDouble(6000.0));
        form->addRow(tr("Cutoff"), c);
        connect(c, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("audiogrid")) {
        auto *cols = new QSpinBox(paramPanel);
        cols->setObjectName(QStringLiteral("fx_audio_grid_cols"));
        cols->setRange(1, 32);
        cols->setValue(p.value(QStringLiteral("gridCols")).toInt(1));
        cols->setToolTip(tr("Parallel taps mixed down (with spread). Higher counts cost more CPU."));
        form->addRow(tr("Grid columns"), cols);
        connect(cols, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
        auto *rows = new QSpinBox(paramPanel);
        rows->setObjectName(QStringLiteral("fx_audio_grid_rows"));
        rows->setRange(1, 32);
        rows->setValue(p.value(QStringLiteral("gridRows")).toInt(1));
        form->addRow(tr("Grid rows"), rows);
        connect(rows, QOverload<int>::of(&QSpinBox::valueChanged), this, [hook](int) { hook(); });
        auto *sp = new QDoubleSpinBox(paramPanel);
        sp->setObjectName(QStringLiteral("fx_audio_spread_ms"));
        sp->setRange(0.0, 120.0);
        sp->setDecimals(2);
        sp->setSuffix(QStringLiteral(" ms"));
        sp->setValue(p.value(QStringLiteral("spreadMs")).toDouble(4.0));
        sp->setToolTip(tr("Max delay between first and last tap. At 1×1 grid this has no effect."));
        form->addRow(tr("Delay spread"), sp);
        connect(sp, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
    } else if (type == QStringLiteral("slowedreverb")) {
        auto *rate = new QDoubleSpinBox(paramPanel);
        rate->setObjectName(QStringLiteral("fx_sr_rate"));
        rate->setRange(0.25, 32.0);
        rate->setDecimals(3);
        rate->setSingleStep(0.05);
        rate->setValue(p.value(QStringLiteral("playbackRate")).toDouble(0.82));
        rate->setToolTip(tr("Timeline vs source: below 1 slows (longer event), above 1 speeds up (shorter event). 1 = normal."));
        form->addRow(tr("Playback rate"), rate);
        connect(rate, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
        auto *mix = new QDoubleSpinBox(paramPanel);
        mix->setObjectName(QStringLiteral("fx_sr_mix"));
        mix->setRange(0.0, 1.0);
        mix->setDecimals(2);
        mix->setSingleStep(0.02);
        mix->setValue(p.value(QStringLiteral("reverbMix")).toDouble(0.38));
        form->addRow(tr("Reverb mix"), mix);
        connect(mix, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
        auto *delay = new QDoubleSpinBox(paramPanel);
        delay->setObjectName(QStringLiteral("fx_sr_delay"));
        delay->setRange(0.0, 800.0);
        delay->setDecimals(0);
        delay->setSuffix(QStringLiteral(" ms"));
        delay->setValue(p.value(QStringLiteral("reverbDelayMs")).toDouble(140.0));
        form->addRow(tr("Reverb delay"), delay);
        connect(delay, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
        auto *decay = new QDoubleSpinBox(paramPanel);
        decay->setObjectName(QStringLiteral("fx_sr_decay"));
        decay->setRange(0.0, 1.0);
        decay->setDecimals(3);
        decay->setSingleStep(0.01);
        decay->setValue(p.value(QStringLiteral("reverbDecay")).toDouble(0.58));
        form->addRow(tr("Reverb decay"), decay);
        connect(decay, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
        auto *damp = new QDoubleSpinBox(paramPanel);
        damp->setObjectName(QStringLiteral("fx_sr_damp"));
        damp->setRange(0.0, 1.0);
        damp->setDecimals(3);
        damp->setSingleStep(0.01);
        damp->setValue(p.value(QStringLiteral("reverbDamp")).toDouble(0.32));
        form->addRow(tr("Reverb damp"), damp);
        connect(damp, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
        auto *gain = new QDoubleSpinBox(paramPanel);
        gain->setObjectName(QStringLiteral("fx_sr_gain"));
        gain->setRange(0.0, 8.0);
        gain->setDecimals(3);
        gain->setSingleStep(0.05);
        gain->setValue(p.value(QStringLiteral("reverbGain")).toDouble(0.95));
        form->addRow(tr("Reverb gain"), gain);
        connect(gain, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [hook](double) { hook(); });
        auto *lockDur = new QCheckBox(tr("Keep event length on timeline (no auto stretch/shrink)"), paramPanel);
        lockDur->setObjectName(QStringLiteral("fx_sr_lock_dur"));
        lockDur->setChecked(p.value(QStringLiteral("lockTimelineDuration")).toBool(false));
        lockDur->setToolTip(tr("When off, changing playback rate resizes the clip. When on, rate still affects playback inside the fixed event."));
        form->addRow(lockDur);
        connect(lockDur, &QCheckBox::toggled, this, [hook](bool) { hook(); });
    } else {
        form->addRow(new QLabel(tr("No editor for type \"%1\"").arg(type), paramPanel));
    }

    const QStringList animKeys = animatableParamKeys(type);
    if (!animKeys.isEmpty()) {
        auto *hint =
            new QLabel(tr("Automation: one horizontal lane per parameter (envelope-style). "
                          "Green dashed lines mark clip start/end (t = 0 and 1 along the clip). "
                          "Markers may sit outside [0,1] — useful after trimming — and remain draggable. "
                          "Click a diamond to load that keyframe into the controls below for editing."),
                       paramPanel);
        hint->setWordWrap(true);
        form->addRow(hint);

        QStringList laneLabels;
        laneLabels.reserve(animKeys.size());
        for (const QString &k : animKeys) {
            laneLabels.append(automationLabelForKey(k));
        }

        auto *ml = new FxAutomationMultiLaneWidget(paramPanel);
        ml->setObjectName(QStringLiteral("fx_param_automation_multilane"));
        ml->setParamKeys(animKeys, laneLabels);
        ml->setKeyframes(p.value(QStringLiteral("automation")).toArray());
        ml->onEdited = [hook]() { hook(); };
        ml->onKeyframeSelected = [this](int idx) {
            fxAutomationSelectedKeyframe = idx;
            if (idx >= 0) {
                syncWidgetsFromAutomationKeyframe(idx);
            }
        };
        form->addRow(tr("Envelopes"), ml);

        auto *tRow = new QWidget(paramPanel);
        auto *th = new QHBoxLayout(tRow);
        th->setContentsMargins(0, 0, 0, 0);
        auto *timeSpin = new QDoubleSpinBox(tRow);
        timeSpin->setRange(-100.0, 100.0);
        timeSpin->setDecimals(5);
        timeSpin->setSingleStep(0.02);
        timeSpin->setValue(0.5);
        timeSpin->setToolTip(
            tr("Time of a new keyframe (t). Inside the clip, playback uses 0…1; keys outside stay editable."));
        auto *addBtn = new QPushButton(tr("Add keyframe at t"), tRow);
        auto *delBtn = new QPushButton(tr("Remove selected"), tRow);
        auto *copyBtn = new QPushButton(tr("Copy envelopes"), tRow);
        auto *pasteBtn = new QPushButton(tr("Paste envelopes"), tRow);
        th->addWidget(timeSpin);
        th->addWidget(addBtn);
        th->addWidget(delBtn);
        th->addWidget(copyBtn);
        th->addWidget(pasteBtn);
        th->addStretch(1);
        form->addRow(tr("Keyframes"), tRow);

        connect(addBtn, &QPushButton::clicked, this, [this, type, timeSpin, ml, hook]() {
            applyParamsFromPanelToChain();
            if (selectedFxRow < 0 || selectedFxRow >= workingChain.size()) {
                return;
            }
            QJsonObject slot = workingChain.at(selectedFxRow).toObject();
            QJsonObject bp = slot.value(QStringLiteral("params")).toObject();
            QJsonArray arr = bp.value(QStringLiteral("automation")).toArray();
            QJsonObject row;
            row.insert(QStringLiteral("t"), timeSpin->value());
            for (const QString &k : animatableParamKeys(type)) {
                if (bp.contains(k)) {
                    row.insert(k, bp.value(k));
                }
            }
            arr.append(row);
            bp.insert(QStringLiteral("automation"), arr);
            slot.insert(QStringLiteral("params"), bp);
            workingChain[selectedFxRow] = slot;
            ml->setKeyframes(arr);
            hook();
        });
        connect(delBtn, &QPushButton::clicked, this, [this, ml, hook]() {
            if (ml->removeSelectedKeyframe()) {
                fxAutomationSelectedKeyframe = ml->selectedKeyframeIndex();
                hook();
            }
        });
        connect(copyBtn, &QPushButton::clicked, this, [ml]() {
            const QJsonDocument doc(ml->keyframes());
            auto *mime = new QMimeData;
            mime->setData(QString::fromLatin1(kFxAutomationMime), doc.toJson(QJsonDocument::Compact));
            mime->setText(QString::fromUtf8(doc.toJson(QJsonDocument::Indented)));
            QGuiApplication::clipboard()->setMimeData(mime);
        });
        connect(pasteBtn, &QPushButton::clicked, this, [this, ml, hook]() {
            const QClipboard *clip = QGuiApplication::clipboard();
            QByteArray raw;
            if (const QMimeData *md = clip->mimeData()) {
                if (md->hasFormat(QString::fromLatin1(kFxAutomationMime))) {
                    raw = md->data(QString::fromLatin1(kFxAutomationMime));
                }
            }
            if (raw.isEmpty()) {
                raw = clip->text().toUtf8();
            }
            QJsonParseError err{};
            const QJsonDocument doc = QJsonDocument::fromJson(raw, &err);
            QJsonArray arr;
            if (doc.isArray()) {
                arr = doc.array();
            }
            if (arr.isEmpty()) {
                QMessageBox::information(this,
                                         tr("Paste envelopes"),
                                         tr("Clipboard does not contain a valid automation JSON array."));
                return;
            }
            ml->setKeyframes(arr);
            hook();
        });
    }

    suppressParamRefresh = 0;
}

void FxChainEditorWidget::onAddClicked()
{
    const QString type = addCombo->currentData().toString();
    applyParamsFromPanelToChain();
    QJsonObject slot;
    slot.insert(QStringLiteral("type"), type);
    if (mediaKind == MediaKind::Video) {
        slot.insert(QStringLiteral("params"), defaultParamsForVideoType(type));
    } else {
        slot.insert(QStringLiteral("params"), defaultParamsForAudioType(type));
    }
    workingChain.append(slot);
    selectedFxRow = workingChain.size() - 1;
    rebuildChainStrip();
    rebuildParamPanel();
    emit chainChanged(QJsonArray(workingChain));
}

void FxChainEditorWidget::onRemoveClicked()
{
    if (selectedFxRow < 0 || selectedFxRow >= workingChain.size()) {
        return;
    }
    applyParamsFromPanelToChain();
    workingChain.removeAt(selectedFxRow);
    if (workingChain.isEmpty()) {
        selectedFxRow = -1;
    } else {
        selectedFxRow = std::min(selectedFxRow, workingChain.size() - 1);
    }
    rebuildChainStrip();
    rebuildParamPanel();
    emit chainChanged(QJsonArray(workingChain));
}

void FxChainEditorWidget::refreshAutomationUi()
{
    if (!paramPanel) {
        return;
    }
    if (FxAutomationMultiLaneWidget *ml = findAutomationMultiLane(paramPanel)) {
        ml->refreshTimeScale();
    }
}

void FxChainEditorWidget::syncWidgetsFromAutomationKeyframe(int idx)
{
    if (!paramPanel || selectedFxRow < 0) {
        return;
    }
    FxAutomationMultiLaneWidget *ml = findAutomationMultiLane(paramPanel);
    if (!ml || idx < 0 || idx >= ml->keyframes().size()) {
        return;
    }
    const QJsonObject kf = ml->keyframes().at(idx).toObject();
    const QString type = effectTypeAt(selectedFxRow);
    suppressParamRefresh++;

    auto setModeCombo = [&](const QString &objName, const QString &key) {
        if (!kf.contains(key)) {
            return;
        }
        auto *cb = paramPanel->findChild<QComboBox *>(objName);
        if (!cb) {
            return;
        }
        const QString data = kf.value(key).toString();
        const int ix = cb->findData(data);
        if (ix >= 0) {
            cb->setCurrentIndex(ix);
        }
    };

    if (type == QStringLiteral("pan")) {
        setModeCombo(QStringLiteral("fx_pan_mode"), QStringLiteral("mode"));
        if (kf.contains(QStringLiteral("panX"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_pan_panX"))) {
                w->setValue(kf.value(QStringLiteral("panX")).toDouble());
            }
        }
        if (kf.contains(QStringLiteral("panY"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_pan_panY"))) {
                w->setValue(kf.value(QStringLiteral("panY")).toDouble());
            }
        }
        if (kf.contains(QStringLiteral("zoom"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_pan_zoom"))) {
                w->setValue(kf.value(QStringLiteral("zoom")).toDouble());
            }
        }
    } else if (type == QStringLiteral("cammotion")) {
        setModeCombo(QStringLiteral("fx_cammotion_mode"), QStringLiteral("mode"));
        if (kf.contains(QStringLiteral("ease"))) {
            if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_cammotion_ease"))) {
                w->setChecked(kf.value(QStringLiteral("ease")).toBool());
            }
        }
        if (kf.contains(QStringLiteral("timeScale"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_cammotion_timescale"))) {
                w->setValue(kf.value(QStringLiteral("timeScale")).toDouble());
            }
        }
    } else if (type == QStringLiteral("mirror")) {
        if (kf.contains(QStringLiteral("horizontal"))) {
            if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_mirror_h"))) {
                w->setChecked(kf.value(QStringLiteral("horizontal")).toBool());
            }
        }
        if (kf.contains(QStringLiteral("vertical"))) {
            if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_mirror_v"))) {
                w->setChecked(kf.value(QStringLiteral("vertical")).toBool());
            }
        }
    } else if (type == QStringLiteral("blur")) {
        if (kf.contains(QStringLiteral("radius"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_blur_radius"))) {
                w->setValue(kf.value(QStringLiteral("radius")).toInt());
            }
        }
    } else if (type == QStringLiteral("greenscreen")) {
        if (kf.contains(QStringLiteral("keyR")) && kf.contains(QStringLiteral("keyG")) && kf.contains(QStringLiteral("keyB"))) {
            if (auto *sw = paramPanel->findChild<QFrame *>(QStringLiteral("fx_chroma_swatch"))) {
                const QColor c(kf.value(QStringLiteral("keyR")).toInt(), kf.value(QStringLiteral("keyG")).toInt(),
                               kf.value(QStringLiteral("keyB")).toInt());
                sw->setProperty("keyColor", c);
                sw->setStyleSheet(
                    QStringLiteral("background:%1; border:1px solid #555; border-radius:4px;").arg(c.name()));
            }
        }
        if (kf.contains(QStringLiteral("tolerance"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_chroma_tol"))) {
                w->setValue(kf.value(QStringLiteral("tolerance")).toInt());
            }
        }
        if (kf.contains(QStringLiteral("softness"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_chroma_soft"))) {
                w->setValue(kf.value(QStringLiteral("softness")).toInt());
            }
        }
    } else if (type == QStringLiteral("edges")) {
        if (kf.contains(QStringLiteral("threshold"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_edges_th"))) {
                w->setValue(kf.value(QStringLiteral("threshold")).toInt());
            }
        }
    } else if (type == QStringLiteral("noise")) {
        if (kf.contains(QStringLiteral("amount"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_noise_amt"))) {
                w->setValue(kf.value(QStringLiteral("amount")).toDouble());
            }
        }
    } else if (type == QStringLiteral("vhs")) {
        if (kf.contains(QStringLiteral("rgbShift"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_vhs_shift"))) {
                w->setValue(kf.value(QStringLiteral("rgbShift")).toInt());
            }
        }
        if (kf.contains(QStringLiteral("bleed"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_vhs_bleed"))) {
                w->setValue(kf.value(QStringLiteral("bleed")).toDouble());
            }
        }
    } else if (type == QStringLiteral("glow")) {
        if (kf.contains(QStringLiteral("strength"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_glow_str"))) {
                w->setValue(kf.value(QStringLiteral("strength")).toDouble());
            }
        }
    } else if (type == QStringLiteral("eventgrid")) {
        if (kf.contains(QStringLiteral("gridCols"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_grid_cols"))) {
                w->setValue(kf.value(QStringLiteral("gridCols")).toInt());
            }
        }
        if (kf.contains(QStringLiteral("gridRows"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_grid_rows"))) {
                w->setValue(kf.value(QStringLiteral("gridRows")).toInt());
            }
        }
        if (kf.contains(QStringLiteral("centerGrid"))) {
            if (auto *w = paramPanel->findChild<QCheckBox *>(QStringLiteral("fx_grid_center"))) {
                w->setChecked(kf.value(QStringLiteral("centerGrid")).toBool());
            }
        }
    } else if (type == QStringLiteral("audiogrid")) {
        if (kf.contains(QStringLiteral("gridCols"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_audio_grid_cols"))) {
                w->setValue(kf.value(QStringLiteral("gridCols")).toInt());
            }
        }
        if (kf.contains(QStringLiteral("gridRows"))) {
            if (auto *w = paramPanel->findChild<QSpinBox *>(QStringLiteral("fx_audio_grid_rows"))) {
                w->setValue(kf.value(QStringLiteral("gridRows")).toInt());
            }
        }
        if (kf.contains(QStringLiteral("spreadMs"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_audio_spread_ms"))) {
                w->setValue(kf.value(QStringLiteral("spreadMs")).toDouble());
            }
        }
    } else if (type == QStringLiteral("softclip")) {
        if (kf.contains(QStringLiteral("drive"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sc_drive"))) {
                w->setValue(kf.value(QStringLiteral("drive")).toDouble());
            }
        }
    } else if (type == QStringLiteral("lowpass")) {
        if (kf.contains(QStringLiteral("cutoffHz"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_lp_cut"))) {
                w->setValue(kf.value(QStringLiteral("cutoffHz")).toDouble());
            }
        }
    } else if (type == QStringLiteral("slowedreverb")) {
        if (kf.contains(QStringLiteral("playbackRate"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_rate"))) {
                w->setValue(kf.value(QStringLiteral("playbackRate")).toDouble());
            }
        }
        if (kf.contains(QStringLiteral("reverbMix"))) {
            if (auto *w = paramPanel->findChild<QDoubleSpinBox *>(QStringLiteral("fx_sr_mix"))) {
                w->setValue(kf.value(QStringLiteral("reverbMix")).toDouble());
            }
        }
    }

    suppressParamRefresh--;
}

#include "fxchaineditorwidget.moc"
