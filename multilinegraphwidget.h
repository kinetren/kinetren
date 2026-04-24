#ifndef MULTILINEGRAPHWIDGET_H
#define MULTILINEGRAPHWIDGET_H

#include <deque>
#include <vector>

#include <QColor>
#include <QString>
#include <QVector>
#include <QWidget>

enum class MultiLineAxisStyle {
    /// 0–100% vertical scale per series (performance panel style).
    NormalizedPercentGrid,
    /// Shared linear min/max over all samples (render speed chart style).
    LinearAutoRange
};

class MultiLineGraphWidget : public QWidget
{
public:
    explicit MultiLineGraphWidget(const QString &title,
                                  const QVector<QString> &seriesNames,
                                  const QVector<QColor> &seriesColors,
                                  const QVector<double> &seriesMax,
                                  MultiLineAxisStyle axisStyle = MultiLineAxisStyle::NormalizedPercentGrid,
                                  int maxSampleHistory = 180,
                                  QWidget *parent = nullptr);

    void clearSamples();

    void addSample(const QVector<double> &values);

protected:
    void paintEvent(QPaintEvent *event) override;

private:
    struct PinnedLegend {
        qreal sourceY = 0.0;
        qreal pinnedY = 0.0;
        QString text;
        QColor color;
    };

    static QVector<PinnedLegend> pinLegends(const QVector<PinnedLegend> &raw,
                                            qreal top,
                                            qreal bottom,
                                            qreal minGap);

    void paintPercentStyle(QPainter &p, const QRect &rect);
    void paintLinearStyle(QPainter &p, const QRect &rect);

    QString m_title;
    QVector<QString> m_names;
    QVector<QColor> m_colors;
    QVector<double> m_seriesMax;
    QVector<double> m_adaptiveScale;
    std::vector<std::deque<double>> m_series;
    MultiLineAxisStyle m_axisStyle = MultiLineAxisStyle::NormalizedPercentGrid;
    int m_maxSampleHistory = 180;
};

#endif
