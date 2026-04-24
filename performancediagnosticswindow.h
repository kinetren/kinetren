#ifndef PERFORMANCEDIAGNOSTICSWINDOW_H
#define PERFORMANCEDIAGNOSTICSWINDOW_H

#include <QWidget>

class MultiLineGraphWidget;
class QTextBrowser;

/// Utility window (Morpher-style): log + graphs that do **not** duplicate the main PerformanceWidget charts.
class PerformanceDiagnosticsWindow : public QWidget
{
public:
    explicit PerformanceDiagnosticsWindow(QWidget *parent = nullptr);

    /// Rates / levels not shown on the main panel (no FPS / hit% / avcpp totals there).
    void ingestDiagnosticsSamples(double noReadDropsPerSec,
                                   double resourceCacheMissesPerSec,
                                   double cacheOccupancyPct,
                                   double audioCacheMissesPerSec);

    void appendProblem(const QString &line);
    void clearProblems();

private:
    QTextBrowser *logBrowser = nullptr;
    MultiLineGraphWidget *churnGraph = nullptr;
};

#endif
