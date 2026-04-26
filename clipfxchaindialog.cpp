#include "clipfxchaindialog.h"

#include "timelineeditor.h"
#include "bc_fx.h"

#include <QComboBox>
#include <QDialogButtonBox>
#include <QHBoxLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QListWidget>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSplitter>
#include <QVBoxLayout>

namespace {

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

} // namespace

ClipFxChainDialog::ClipFxChainDialog(QWidget *parent,
                                     TimelineEditor *timeline,
                                     int clipId,
                                     MediaKind kind,
                                     const QString &focusHint,
                                     const QSize &projectOutputSize)
    : QDialog(parent)
    , timeline(timeline)
    , clipId(clipId)
    , kind(kind)
    , projectOutputSize(projectOutputSize)
{
    setWindowTitle(kind == MediaKind::Video ? tr("Video effects") : tr("Audio effects"));
    setMinimumSize(720, 520);

    if (kind == MediaKind::Video) {
        workingChain = timeline->clipVideoFxChain(clipId);
        if (workingChain.isEmpty()) {
            workingChain = burstcon::defaultVideoFxChain();
        }
    } else {
        workingChain = timeline->clipAudioFxChain(clipId);
    }

    auto *root = new QVBoxLayout(this);
    auto *split = new QSplitter(Qt::Vertical, this);

    auto *top = new QWidget(split);
    auto *topLay = new QHBoxLayout(top);
    fxList = new QListWidget(top);
    fxList->setMinimumWidth(220);
    topLay->addWidget(fxList, 0);

    auto *rightCol = new QVBoxLayout();
    auto *paramLabel = new QLabel(tr("Parameters (JSON):"), top);
    rightCol->addWidget(paramLabel);
    paramEdit = new QPlainTextEdit(top);
    paramEdit->setPlaceholderText(tr("{ \"key\": value, ... }"));
    paramEdit->setTabChangesFocus(true);
    rightCol->addWidget(paramEdit, 1);
    topLay->addLayout(rightCol, 1);

    auto *addRow = new QHBoxLayout();
    addCombo = new QComboBox(top);
    if (kind == MediaKind::Video) {
        const struct {
            const char *id;
            const char *label;
        } items[] = {
            {"pan", "Pan / frame"},
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
    auto *addBtn = new QPushButton(tr("Add"), top);
    auto *removeBtn = new QPushButton(tr("Remove"), top);
    auto *upBtn = new QPushButton(tr("Up"), top);
    auto *downBtn = new QPushButton(tr("Down"), top);
    addRow->addWidget(addCombo, 1);
    addRow->addWidget(addBtn);
    addRow->addWidget(removeBtn);
    addRow->addWidget(upBtn);
    addRow->addWidget(downBtn);

    root->addWidget(split, 1);
    root->addLayout(addRow);

    if (kind == MediaKind::Video) {
        preview = new QLabel(split);
        preview->setMinimumHeight(200);
        preview->setAlignment(Qt::AlignCenter);
        preview->setStyleSheet(QStringLiteral("QLabel { background:#101214; border:1px solid #343a42; }"));
        preview->setText(tr("Preview"));
        split->addWidget(top);
        split->addWidget(preview);
        split->setStretchFactor(0, 3);
        split->setStretchFactor(1, 2);
    } else {
        split->addWidget(top);
    }

    auto *bbox = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    root->addWidget(bbox);

    connect(bbox, &QDialogButtonBox::accepted, this, [this]() {
        saveParamsFromEditor();
        if (kind == MediaKind::Video && workingChain.isEmpty()) {
            workingChain = burstcon::defaultVideoFxChain();
        }
        if (kind == MediaKind::Video) {
            timeline->setClipVideoFxChain(clipId, workingChain);
        } else {
            timeline->setClipAudioFxChain(clipId, workingChain);
        }
        accept();
    });
    connect(bbox, &QDialogButtonBox::rejected, this, &QDialog::reject);

    connect(fxList, &QListWidget::currentRowChanged, this, [this](int) {
        saveParamsFromEditor();
        loadParamsEditor();
        refreshPreview();
    });
    connect(paramEdit, &QPlainTextEdit::textChanged, this, [this]() {
        if (suppressParamSignal) {
            return;
        }
        saveParamsFromEditor();
        refreshPreview();
    });

    connect(addBtn, &QPushButton::clicked, this, [this]() {
        saveParamsFromEditor();
        const QString type = addCombo->currentData().toString();
        appendEffect(type);
    });
    connect(removeBtn, &QPushButton::clicked, this, [this]() {
        const int row = fxList->currentRow();
        if (row < 0 || row >= workingChain.size()) {
            return;
        }
        saveParamsFromEditor();
        workingChain.removeAt(row);
        repopulateList();
        fxList->setCurrentRow(std::min(row, workingChain.size() - 1));
        refreshPreview();
    });
    connect(upBtn, &QPushButton::clicked, this, [this]() {
        const int row = fxList->currentRow();
        if (row <= 0) {
            return;
        }
        saveParamsFromEditor();
        const QJsonValue v = workingChain.at(row);
        workingChain.removeAt(row);
        workingChain.insert(row - 1, v);
        repopulateList();
        fxList->setCurrentRow(row - 1);
        refreshPreview();
    });
    connect(downBtn, &QPushButton::clicked, this, [this]() {
        const int row = fxList->currentRow();
        if (row < 0 || row >= workingChain.size() - 1) {
            return;
        }
        saveParamsFromEditor();
        const QJsonValue v = workingChain.at(row);
        workingChain.removeAt(row);
        workingChain.insert(row + 1, v);
        repopulateList();
        fxList->setCurrentRow(row + 1);
        refreshPreview();
    });

    repopulateList();
    if (focusHint == QStringLiteral("pan")) {
        for (int i = 0; i < workingChain.size(); ++i) {
            const QString t = workingChain.at(i).toObject().value(QStringLiteral("type")).toString();
            if (t == QStringLiteral("pan")) {
                fxList->setCurrentRow(i);
                break;
            }
        }
    } else if (fxList->count() > 0) {
        fxList->setCurrentRow(fxList->count() - 1);
    }
    loadParamsEditor();
    refreshPreview();
}

void ClipFxChainDialog::appendEffect(const QString &typeId)
{
    QJsonObject slot;
    slot.insert(QStringLiteral("type"), typeId);
    if (kind == MediaKind::Video) {
        slot.insert(QStringLiteral("params"), defaultParamsForVideoType(typeId));
    } else {
        slot.insert(QStringLiteral("params"), defaultParamsForAudioType(typeId));
    }
    workingChain.append(slot);
    repopulateList();
    fxList->setCurrentRow(workingChain.size() - 1);
    refreshPreview();
}

void ClipFxChainDialog::repopulateList()
{
    const int prev = fxList->currentRow();
    fxList->clear();
    for (int i = 0; i < workingChain.size(); ++i) {
        const QJsonObject o = workingChain.at(i).toObject();
        const QString type = o.value(QStringLiteral("type")).toString();
        fxList->addItem(QStringLiteral("%1: %2").arg(i + 1).arg(type.isEmpty() ? QStringLiteral("?") : type));
    }
    if (prev >= 0 && prev < fxList->count()) {
        fxList->setCurrentRow(prev);
    }
}

void ClipFxChainDialog::loadParamsEditor()
{
    const int row = fxList->currentRow();
    suppressParamSignal = 1;
    if (row < 0 || row >= workingChain.size()) {
        paramEdit->clear();
        suppressParamSignal = 0;
        return;
    }
    const QJsonObject o = workingChain.at(row).toObject();
    const QJsonObject params = o.value(QStringLiteral("params")).toObject();
    paramEdit->setPlainText(QString::fromUtf8(QJsonDocument(params).toJson(QJsonDocument::Indented)));
    suppressParamSignal = 0;
}

void ClipFxChainDialog::saveParamsFromEditor()
{
    const int row = fxList->currentRow();
    if (row < 0 || row >= workingChain.size()) {
        return;
    }
    QJsonObject slot = workingChain.at(row).toObject();
    const QJsonDocument doc = QJsonDocument::fromJson(paramEdit->toPlainText().toUtf8());
    if (!doc.isObject()) {
        return;
    }
    slot.insert(QStringLiteral("params"), doc.object());
    workingChain[row] = slot;
}

void ClipFxChainDialog::refreshPreview()
{
    if (kind != MediaKind::Video || !preview) {
        return;
    }
    QImage img = timeline->previewStillForClip(clipId);
    if (img.isNull()) {
        preview->setText(tr("No preview frame"));
        return;
    }
    const int pw = std::max(2, std::min(640, projectOutputSize.width() > 0 ? projectOutputSize.width() : 1280));
    const int ph = std::max(2, std::min(360, projectOutputSize.height() > 0 ? projectOutputSize.height() : 720));
    burstcon::VideoBurst vb;
    vb.index = 0;
    vb.quality = burstcon::BurstQuality{img.width(), img.height()};
    vb.frame = img.convertToFormat(QImage::Format_ARGB32);
    burstcon::VideoFxContext ctx{{pw, ph}, 0, {}};
    burstcon::processVideoFxChainInPlace(vb, workingChain, ctx);
    if (vb.frame.isNull()) {
        preview->setText(tr("Effect error"));
        return;
    }
    preview->setPixmap(QPixmap::fromImage(vb.frame.scaled(pw, ph, Qt::KeepAspectRatio, Qt::SmoothTransformation)));
}
