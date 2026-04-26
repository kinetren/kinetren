#include "fxchaineditorwidget.h"
#include "timelineeditor.h"

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
#include <QVariant>

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
                                             {QStringLiteral("softclip"), QStringLiteral("Soft clip")},
                                             {QStringLiteral("lowpass"), QStringLiteral("Low-pass")}};
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
        };
        for (const auto &it : items) {
            addCombo->addItem(QString::fromUtf8(it.label), QString::fromUtf8(it.id));
        }
    } else {
        addCombo->addItem(tr("Soft clip"), QStringLiteral("softclip"));
        addCombo->addItem(tr("Low-pass"), QStringLiteral("lowpass"));
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
    }

    slot.insert(QStringLiteral("params"), p);
    workingChain[selectedFxRow] = slot;
}

void FxChainEditorWidget::rebuildParamPanel()
{
    suppressParamRefresh = 1;
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
    } else {
        form->addRow(new QLabel(tr("No editor for type \"%1\"").arg(type), paramPanel));
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

#include "fxchaineditorwidget.moc"
