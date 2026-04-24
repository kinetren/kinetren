#include "speedchartwidget.h"

#include "multilinegraphwidget.h"

#include <cmath>

#include <QVBoxLayout>

SpeedChartWidget::SpeedChartWidget(QWidget *parent)
    : QWidget(parent)
{
    setMinimumHeight(128);
    setMaximumHeight(200);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);

    auto *lay = new QVBoxLayout(this);
    lay->setContentsMargins(0, 0, 0, 0);
    lay->setSpacing(0);
    m_graph = new MultiLineGraphWidget(QStringLiteral("Speed vs realtime"),
                                       {QStringLiteral("× realtime")},
                                       {QColor(88, 198, 255)},
                                       {6.0},
                                       MultiLineAxisStyle::LinearAutoRange,
                                       480,
                                       this);
    m_graph->setMinimumHeight(128);
    m_graph->setMaximumHeight(200);
    lay->addWidget(m_graph);
}

QSize SpeedChartWidget::sizeHint() const
{
    return QSize(600, 168);
}

QSize SpeedChartWidget::minimumSizeHint() const
{
    return QSize(320, 128);
}

void SpeedChartWidget::clear()
{
    if (m_graph) {
        m_graph->clearSamples();
    }
}

void SpeedChartWidget::addSample(double speedRealtimeFactor)
{
    if (!m_graph || !std::isfinite(speedRealtimeFactor) || speedRealtimeFactor <= 0.0) {
        return;
    }
    m_graph->addSample({speedRealtimeFactor});
}
