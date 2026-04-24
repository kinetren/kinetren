#ifndef PERFORMANCEDIAGNOSTICSWINDOW_H
#define PERFORMANCEDIAGNOSTICSWINDOW_H

#include <QWidget>

class MultiLineGraphWidget;
class QTextBrowser;
/// Utility window (same window flags as Morpher) for cache / decode diagnostics and problem log.
class PerformanceDiagnosticsWindow : public QWidget
{
public:
    explicit PerformanceDiagnosticsWindow(QWidget *parent = nullptr);

    /// Feed the rolling graph (percent-style axes, independent scales per series).
    void ingestGraphSample(double unusedEvictPerSec,
                           double videoDecodePerSec,
                           double resourceHitPct,
                           double cacheEntriesPercent);

    void appendProblem(const QString &line);
    void clearProblems();

private:
    QTextBrowser *logBrowser = nullptr;
    MultiLineGraphWidget *graph = nullptr;
};

#endif
