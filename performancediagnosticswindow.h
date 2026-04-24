#ifndef PERFORMANCEDIAGNOSTICSWINDOW_H
#define PERFORMANCEDIAGNOSTICSWINDOW_H

#include <QWidget>

class QTextBrowser;

/// Utility window (same window flags as Morpher) for diagnostic log only (no duplicate performance graphs).
class PerformanceDiagnosticsWindow : public QWidget
{
public:
    explicit PerformanceDiagnosticsWindow(QWidget *parent = nullptr);

    void appendProblem(const QString &line);
    void clearProblems();

private:
    QTextBrowser *logBrowser = nullptr;
};

#endif
