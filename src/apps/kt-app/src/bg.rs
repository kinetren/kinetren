//! Фоновая ЗАДАЧА — общий каркас (Р.49, п.367).
//!
//! **Различие, из которого всё следует.** В программе есть ДВЕ формы фоновой
//! работы, и путать их дорого:
//!
//! - **задача** — конечная, с прогрессом, отменой и паузой; у неё есть конец,
//!   и пользователь вправе его ждать. Эталон — рендер;
//! - **сервис** — бесконечный поток с очередью, который живёт, пока живёт
//!   проект. Эталон — служба тумбнейлов.
//!
//! `spawn_index_worker` был задачей, ПРИТВОРЯВШЕЙСЯ сервисом: каждый вызов
//! поднимал новый поток, и на одном проекте их набиралось три, а дописывали
//! они в осиротевшие `Arc`, которых никто уже не читал.
//!
//! Каркас выделен ИЗ `RenderJob`, а не написан рядом: копия разошлась бы с
//! оригиналом на первой же правке.
//!
//! **`Drop` гасит поток.** Это лечение болезни, а не удобство: до
//! `WatchdogHandle` переоткрытие проекта оставляло прежние потоки жить вечно.
//! Поток проверяет флаг сам — прервать его снаружи нельзя, и это честно:
//! задача, не спрашивающая «не пора ли остановиться», остановлена быть и не
//! может.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Отправитель прогресса. ВЛАДЕЕТСЯ телом задачи, а не одалживается ему:
/// потребители (тот же экспорт) требуют колбэк `Fn(..) + Send + Sync +
/// 'static` и хранят его у себя, а ссылка такого обещания дать не может.
pub struct BgSend<P>(std::sync::mpsc::Sender<P>);

impl<P> Clone for BgSend<P> {
    fn clone(&self) -> Self {
        BgSend(self.0.clone())
    }
}

impl<P> BgSend<P> {
    /// Отправить шаг. Закрытый канал — не ошибка: значит про задачу забыли,
    /// и она всё равно вот-вот остановится по флагу отмены.
    pub fn send(&self, p: P) {
        let _ = self.0.send(p);
    }
}

/// Рычаги, которые задача ОБЯЗАНА опрашивать сама.
#[derive(Clone)]
pub struct BgCtl {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

impl BgCtl {
    /// «Пора ли заканчивать». Заодно ЖДЁТ, пока снята пауза: пауза — это не
    /// остановка, и спрашивающий про отмену всё равно обязан её уважать.
    pub fn should_stop(&self) -> bool {
        while self.pause.load(Ordering::Acquire) && !self.cancel.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        self.cancel.load(Ordering::Acquire)
    }

    pub fn cancel_flag(&self) -> &Arc<AtomicBool> {
        &self.cancel
    }

    pub fn pause_flag(&self) -> &Arc<AtomicBool> {
        &self.pause
    }
}

/// Запущенная фоновая задача: `P` — шаг прогресса, `R` — итог.
pub struct BgJob<P, R> {
    prog_rx: std::sync::mpsc::Receiver<P>,
    done_rx: std::sync::mpsc::Receiver<R>,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    pub started: std::time::Instant,
    /// Последний пришедший шаг прогресса.
    pub progress: Option<P>,
    /// Итог. `Some` — задача кончилась.
    pub done: Option<R>,
}

impl<P: Send + 'static, R: Send + 'static> BgJob<P, R> {
    /// Запустить. `body` получает рычаги и «отправитель прогресса».
    pub fn spawn<F>(name: &str, body: F) -> BgJob<P, R>
    where
        F: FnOnce(BgCtl, BgSend<P>) -> R + Send + 'static,
    {
        let cancel = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let (prog_tx, prog_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let ctl = BgCtl { cancel: cancel.clone(), pause: pause.clone() };
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let r = body(ctl, BgSend(prog_tx));
                let _ = done_tx.send(r);
            })
            .expect("spawn background job");
        BgJob {
            prog_rx,
            done_rx,
            cancel,
            pause,
            started: std::time::Instant::now(),
            progress: None,
            done: None,
        }
    }

    /// Забрать всё, что пришло. `true` — что-то изменилось (повод на кадр).
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok(p) = self.prog_rx.try_recv() {
            any = true;
            self.progress = Some(p);
        }
        if let Ok(r) = self.done_rx.try_recv() {
            any = true;
            self.done = Some(r);
        }
        any
    }

    /// Задача ещё считает. Именно это, а не `is_some()` у самого джоба,
    /// должно держать перерисовку: иначе окно, оставшееся на экране после
    /// финиша, греет систему круглые сутки (повтор дефекта t38).
    pub fn is_running(&self) -> bool {
        self.done.is_none()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn set_paused(&self, on: bool) {
        self.pause.store(on, Ordering::Release);
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::Acquire)
    }
}

impl<P, R> Drop for BgJob<P, R> {
    fn drop(&mut self) {
        // забыли про задачу — задача обязана закончиться
        self.cancel.store(true, Ordering::Release);
        self.pause.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t398 (Р.49 Б1): каркас задачи — прогресс доезжает, пауза держит,
    /// отмена останавливает, а `Drop` гасит поток даже если про задачу
    /// просто забыли.
    #[test]
    fn t398_bg_job_reports_pauses_cancels_and_dies_with_drop() {
        // 1) прогресс и итог
        let mut job: BgJob<u32, u32> = BgJob::spawn("t398-a", |_ctl, send: BgSend<u32>| {
            for i in 1..=5 {
                send.send(i);
            }
            42
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job.done.is_none() && std::time::Instant::now() < deadline {
            job.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(job.done, Some(42), "итог не доехал");
        assert_eq!(job.progress, Some(5), "прогресс не доехал");
        assert!(!job.is_running(), "законченная задача считается бегущей");

        // 2) отмена: тело спрашивает `should_stop` и выходит
        let steps = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let s = steps.clone();
        let job2: BgJob<(), &'static str> =
            BgJob::spawn("t398-b", move |ctl, _send: BgSend<()>| {
                loop {
                    if ctl.should_stop() {
                        return "stopped";
                    }
                    s.fetch_add(1, Ordering::Release);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
        std::thread::sleep(std::time::Duration::from_millis(40));
        job2.cancel();
        let mut job2 = job2;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job2.done.is_none() && std::time::Instant::now() < deadline {
            job2.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(job2.done, Some("stopped"), "отменённая задача не закончилась");

        // 3) Drop гасит поток: про задачу забыли — она обязана остановиться
        let ticks = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let t = ticks.clone();
        {
            let _job3: BgJob<(), ()> = BgJob::spawn("t398-c", move |ctl, _send: BgSend<()>| {
                while !ctl.should_stop() {
                    t.fetch_add(1, Ordering::Release);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(40));
        } // здесь Drop
        std::thread::sleep(std::time::Duration::from_millis(80));
        let a = ticks.load(Ordering::Acquire);
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert_eq!(
            ticks.load(Ordering::Acquire),
            a,
            "после Drop поток продолжает крутиться — ровно та утечка, ради которой \
             каркас и заведён"
        );
    }

    /// Пауза ДЕРЖИТ, а отмена снимает её и выпускает поток: иначе отменённая
    /// на паузе задача висела бы вечно, а вместе с ней и окно прогресса.
    #[test]
    fn t398b_cancel_releases_a_paused_job() {
        let ticks = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let t = ticks.clone();
        let mut job: BgJob<(), u32> = BgJob::spawn("t398-d", move |ctl, _send: BgSend<()>| {
            let mut n = 0;
            while !ctl.should_stop() {
                n += 1;
                t.fetch_add(1, Ordering::Release);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            n
        });
        std::thread::sleep(std::time::Duration::from_millis(40));
        job.set_paused(true);
        std::thread::sleep(std::time::Duration::from_millis(60));
        let held = ticks.load(Ordering::Acquire);
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert_eq!(ticks.load(Ordering::Acquire), held, "пауза не держит");

        job.cancel();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job.done.is_none() && std::time::Instant::now() < deadline {
            job.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(job.done.is_some(), "отменённая на паузе задача не вышла — окно висело бы вечно");
    }
}
