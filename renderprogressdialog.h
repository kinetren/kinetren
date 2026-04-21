#ifndef RENDERPROGRESSDIALOG_H
#define RENDERPROGRESSDIALOG_H

#include <QDialog>

#include <atomic>
#include <memory>

#include "projectsettings.h"

class QPlainTextEdit;
class QProgressBar;
class QLabel;
class QPushButton;
class QToolButton;
class SpeedChartWidget;

class RenderProgressDialog : public QDialog
{
    Q_OBJECT

public:
    explicit RenderProgressDialog(QWidget *parent = nullptr);

    std::shared_ptr<std::atomic<bool>> cancelFlag() const { return m_cancelFlag; }
    std::shared_ptr<std::atomic<bool>> pauseFlag() const { return m_pauseFlag; }

    void appendLog(const QString &line);
    void setExportInfo(const ProjectSettings &settings);
    void setProgress(int value, int maximum);
    void setProgressView(const QString &phase,
                         int done,
                         int total,
                         const QString &elapsed,
                         const QString &remaining,
                         const QString &outputLen,
                         double speedRealtimeFactor);
    void setRunning(bool running);
    void setFinished(bool ok, const QString &summary);

private:
    void onPauseClicked();
    void onCancelClicked();
    void onLogToggle(bool checked);
    void updateSpeedStatsLine();

    QPlainTextEdit *m_log = nullptr;
    QToolButton *m_logToggle = nullptr;
    QProgressBar *m_bar = nullptr;
    QLabel *m_outResolution = nullptr;
    QLabel *m_outFps = nullptr;
    QLabel *m_outVideo = nullptr;
    QLabel *m_outAudio = nullptr;
    QLabel *m_outMux = nullptr;
    QLabel *m_valPhase = nullptr;
    QLabel *m_valProgress = nullptr;
    QLabel *m_valElapsed = nullptr;
    QLabel *m_valRemaining = nullptr;
    QLabel *m_valOutput = nullptr;
    QLabel *m_speedStats = nullptr;
    SpeedChartWidget *m_chart = nullptr;
    QPushButton *m_pauseBtn = nullptr;
    QPushButton *m_cancelBtn = nullptr;
    QPushButton *m_okBtn = nullptr;

    std::shared_ptr<std::atomic<bool>> m_cancelFlag;
    std::shared_ptr<std::atomic<bool>> m_pauseFlag;

    double m_speedSum = 0.0;
    int m_speedCount = 0;
    double m_speedMin = 0.0;
    double m_speedMax = 0.0;
    double m_speedCurrent = 0.0;
    bool m_haveSpeedStats = false;
};

#endif
