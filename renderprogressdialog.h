#ifndef RENDERPROGRESSDIALOG_H
#define RENDERPROGRESSDIALOG_H

#include <QDialog>

#include <atomic>
#include <memory>

class QPlainTextEdit;
class QProgressBar;
class QLabel;
class QPushButton;

class RenderProgressDialog : public QDialog
{
    Q_OBJECT

public:
    explicit RenderProgressDialog(QWidget *parent = nullptr);

    std::shared_ptr<std::atomic<bool>> cancelFlag() const { return m_cancelFlag; }
    std::shared_ptr<std::atomic<bool>> pauseFlag() const { return m_pauseFlag; }

    void appendLog(const QString &line);
    void setProgress(int value, int maximum);
    void setStatusHtml(const QString &html);
    void setRunning(bool running);
    void setFinished(bool ok, const QString &summary);

private:
    void onPauseClicked();
    void onCancelClicked();

    QPlainTextEdit *m_log = nullptr;
    QProgressBar *m_bar = nullptr;
    QLabel *m_status = nullptr;
    QPushButton *m_pauseBtn = nullptr;
    QPushButton *m_cancelBtn = nullptr;
    QPushButton *m_okBtn = nullptr;

    std::shared_ptr<std::atomic<bool>> m_cancelFlag;
    std::shared_ptr<std::atomic<bool>> m_pauseFlag;
};

#endif
