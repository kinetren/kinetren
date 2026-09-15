//! Офлайн-анализ материала (Р.49, п.372).
//!
//! Одна подсистема на «посчитай что-нибудь по файлу целиком»: громкость
//! источника и огибающая для автодакинга.
//!
//! Синхронизации по звуку здесь НЕТ, и это не упущение: она считается по
//! пирамиде пиков, которая уже лежит в памяти, — миллисекунды без единого
//! декода. Задача, которая не читает файл, в фоновой подсистеме не нуждается:
//! окно прогресса и очередь были бы церемонией вокруг мгновенной работы.
//!
//! **Гейт «одна задача».** Анализ читает файлы целиком и декодирует их;
//! запустить три сразу — значит отобрать диск и процессор у
//! ВОСПРОИЗВЕДЕНИЯ, которое обещает реальное время (инвариант 6). Поэтому
//! бежит ровно одна задача, остальные ждут в очереди.
//!
//! **Очередь с дедупом.** Человек жмёт «посчитай» дважды — это одна работа,
//! а не две. Дубль в очереди означал бы второй полный декод того же файла.
//!
//! **Негативный кэш.** Задача, которая не вышла (файла нет, формат не
//! читается), НЕ переставляется в очередь сама: иначе программа вечно
//! долбится в недоступный файл. Причина запоминается и показывается; повтор
//! — только по прямой просьбе.
//!
//! **Показ НЕ ставит заявку.** Подсказка карточки бина, наведение на клип,
//! открытие окна — ничто из этого не запускает анализ. Это прямая родня
//! инварианта 8 («скролл/зум не порождает декод-запросов»): иначе проведя
//! мышью по бину, человек запускал бы декод десятка файлов.

use kt_model::MediaId;

/// Что посчитать.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AnalysisKind {
    /// Громкость медиа: LUFS, dBTP, LRA.
    Loudness(MediaId),
    /// Автодакинг: огибающая ведущей дорожки. Долг Р.48 (п.376) — раньше
    /// считался СИНХРОННО в UI-потоке, и на длинном монтаже окно замирало
    /// на десятки секунд без единого признака жизни.
    Duck,
}

impl AnalysisKind {
    /// Медиа, которые задача читает. Нужен стейл-защите: подменили файл —
    /// незаконченная задача про него больше не про этот проект.
    pub fn media(self) -> Vec<MediaId> {
        match self {
            AnalysisKind::Loudness(m) => vec![m],
            AnalysisKind::Duck => Vec::new(),
        }
    }
}

/// Итог задачи.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AnalysisOut {
    Loudness { media: MediaId, lufs: Option<f64>, dbtp: f64, lra: f64 },
    /// Интервалы, где ведущая дорожка звучит (тики), и число ключей.
    Duck { spans: usize },
}

/// Шаг прогресса: доля 0..1.
pub type AnalysisProgress = f32;

type Job = crate::bg::BgJob<AnalysisProgress, Result<AnalysisOut, String>>;

/// Состояние подсистемы.
#[derive(Default)]
pub struct Analysis {
    running: Option<(AnalysisKind, Job)>,
    queue: std::collections::VecDeque<AnalysisKind>,
    /// Негативный кэш: что не вышло и почему.
    failed: std::collections::HashMap<AnalysisKind, String>,
    /// Окно прогресса открыто.
    pub open: bool,
    /// Ревизия модели, на которой ставилась текущая задача: результат по
    /// подменённому медиа применять нельзя.
    rev: u64,
    /// Готовые тела задач, поставленных через `request_with`.
    #[allow(clippy::type_complexity)]
    bodies: std::collections::HashMap<AnalysisKind, Box<AnalysisBody>>,
}

type AnalysisBody = dyn FnOnce(crate::bg::BgCtl, crate::bg::BgSend<AnalysisProgress>) -> Result<AnalysisOut, String>
    + Send;

impl Analysis {
    /// Что считается прямо сейчас.
    pub fn current(&self) -> Option<AnalysisKind> {
        self.running.as_ref().map(|(k, _)| *k)
    }

    /// Доля выполнения текущей задачи.
    pub fn progress(&self) -> f32 {
        self.running.as_ref().and_then(|(_, j)| j.progress).unwrap_or(0.0)
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// ЖИВА ли работа. Именно это держит перерисовку — не `is_some()` у
    /// окна: окно, оставшееся на экране после финиша, грело бы систему
    /// круглые сутки (повтор дефекта t38).
    pub fn is_running(&self) -> bool {
        self.running.as_ref().is_some_and(|(_, j)| j.is_running())
    }

    /// Почему задача не вышла (негативный кэш).
    pub fn failure(&self, k: AnalysisKind) -> Option<&str> {
        self.failed.get(&k).map(|s| s.as_str())
    }

    /// Поставить задачу С ГОТОВЫМ ТЕЛОМ.
    ///
    /// Нужна там, где считать умеет только вызывающий: у дакинга тело несёт
    /// снимок проекта и настройки диалога, и подсистеме про них знать
    /// незачем. Гейт «одна задача» и очередь работают так же.
    /// `retry` снимает негативный кэш: «попробуй ещё раз» — прямая просьба
    /// человека, а сама программа в недоступный файл не долбится.
    pub fn request_with<F>(&mut self, k: AnalysisKind, rev: u64, retry: bool, body: F)
    where
        F: FnOnce(crate::bg::BgCtl, crate::bg::BgSend<AnalysisProgress>) -> Result<AnalysisOut, String>
            + Send
            + 'static,
    {
        if retry {
            self.failed.remove(&k);
        } else if self.failed.contains_key(&k) {
            return; // уже пробовали и не вышло
        }
        if self.current() == Some(k) || self.queue.contains(&k) {
            return; // дедуп: тот же файл второй раз декодировать незачем
        }
        self.bodies.insert(k, Box::new(body));
        self.queue.push_back(k);
        self.rev = rev;
        self.open = true;
    }

    /// Отменить всё: текущую задачу и очередь.
    pub fn cancel_all(&mut self) {
        if let Some((_, j)) = &self.running {
            j.cancel();
        }
        self.queue.clear();
    }

    /// Двинуть подсистему: забрать готовое, запустить следующую.
    /// `spawn` строит тело задачи — подсистема не знает, как считать.
    ///
    /// Возвращает готовый результат (если он есть) — вместе с ревизией, на
    /// которой задача ставилась: применять его к другой модели нельзя.
    pub fn poll<F>(&mut self, spawn: F) -> Option<(AnalysisKind, u64, Result<AnalysisOut, String>)>
    where
        F: FnOnce(AnalysisKind) -> Job,
    {
        let mut out = None;
        if let Some((kind, job)) = &mut self.running {
            job.poll();
            if let Some(r) = job.done.take() {
                let kind = *kind;
                if let Err(e) = &r {
                    self.failed.insert(kind, e.clone());
                }
                out = Some((kind, self.rev, r));
                self.running = None;
            }
        }
        // гейт «одна задача»: анализ декодирует файлы целиком, и три сразу
        // отняли бы диск у воспроизведения, которое обещает реальное время
        if self.running.is_none()
            && let Some(next) = self.queue.pop_front()
        {
            let job = match self.bodies.remove(&next) {
                Some(body) => crate::bg::BgJob::spawn("analysis", body),
                None => spawn(next),
            };
            self.running = Some((next, job));
        }
        if self.running.is_none() && self.queue.is_empty() {
            self.open = false;
        }
        out
    }
}

/// Тело задачи «по умолчанию» — для видов, которые подсистема умеет сама.
/// Сейчас таких нет: и громкость, и синхронизация считаются там, где есть
/// доступ к источникам и пирамидам, и приходят через `request_with`.
pub fn spawn_default(_k: AnalysisKind) -> Job {
    crate::bg::BgJob::spawn("analysis-noop", |_ctl, _send: crate::bg::BgSend<AnalysisProgress>| {
        Err("no body".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тело-пустышка: сразу отдаёт заданный итог.
    fn done(out: Result<AnalysisOut, String>) -> Job {
        crate::bg::BgJob::spawn("t408", move |_ctl, _send: crate::bg::BgSend<f32>| out)
    }

    fn wait(a: &mut Analysis, out: Result<AnalysisOut, String>) -> Option<(AnalysisKind, u64, Result<AnalysisOut, String>)> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(r) = a.poll(|_| done(out.clone())) {
                return Some(r);
            }
            if std::time::Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Тест 408 (Р.49 Б6): гейт «одна задача» и очередь с дедупом.
    ///
    /// Анализ декодирует файлы целиком: три задачи разом отобрали бы диск у
    /// воспроизведения, которое обещает реальное время (инвариант 6). А дубль
    /// в очереди означал бы второй полный декод одного и того же файла.
    #[test]
    fn t408_one_job_at_a_time_and_queue_dedups() {
        let mut a = Analysis::default();
        let k1 = AnalysisKind::Loudness(MediaId(1));
        let k2 = AnalysisKind::Loudness(MediaId(2));
        let body = |out: Result<AnalysisOut, String>| {
            move |_ctl: crate::bg::BgCtl, _s: crate::bg::BgSend<f32>| out
        };
        let ok = Ok(AnalysisOut::Loudness {
            media: MediaId(1),
            lufs: Some(-20.0),
            dbtp: -6.0,
            lra: 3.0,
        });
        a.request_with(k1, 1, false, body(ok.clone()));
        a.request_with(k1, 1, false, body(ok.clone())); // тот же запрос — не работа
        a.request_with(k2, 1, false, body(ok.clone()));
        assert_eq!(a.queued(), 2, "дубль попал в очередь: {}", a.queued());

        // первая берётся в работу, вторая ждёт
        a.poll(|_| done(ok.clone()));
        assert_eq!(a.current(), Some(k1), "в работу взята не первая задача");
        assert_eq!(a.queued(), 1, "в работу взяты обе разом — гейт не сработал");

        let (kind, rev, r) = wait(&mut a, ok.clone()).expect("первая задача не закончилась");
        assert_eq!(kind, k1);
        assert_eq!(rev, 1, "ревизия постановки потеряна");
        assert!(r.is_ok());
        let (kind, _, _) = wait(&mut a, ok.clone()).expect("вторая задача не закончилась");
        assert_eq!(kind, k2, "очередь не дошла до второй задачи");
        assert!(!a.is_running() && a.queued() == 0);
        assert!(!a.open, "окно прогресса осталось открытым на пустой очереди");
    }

    /// Тест 409 (Р.49 Б6): негативный кэш. Задача, которая не вышла, НЕ
    /// переставляется сама — иначе программа вечно долбится в недоступный
    /// файл; повтор только по прямой просьбе.
    #[test]
    fn t409_failure_is_remembered_and_not_retried_by_itself() {
        let mut a = Analysis::default();
        let k = AnalysisKind::Loudness(MediaId(3));
        let err = |_c: crate::bg::BgCtl, _s: crate::bg::BgSend<f32>| Err("нет файла".to_string());
        a.request_with(k, 1, false, err);
        let (_, _, r) = wait(&mut a, Err("нет файла".into())).expect("задача не закончилась");
        assert!(r.is_err());
        assert_eq!(a.failure(k), Some("нет файла"), "причина отказа не запомнена");

        // сама себя не переставляет
        a.request_with(k, 1, false, err);
        assert_eq!(a.queued(), 0, "программа снова полезла в недоступный файл");

        // а по прямой просьбе — пробует
        a.request_with(k, 1, true, err);
        assert_eq!(a.queued(), 1, "«попробуй ещё раз» не сработало");
        assert!(a.failure(k).is_none(), "негативный кэш не снят повтором");
    }

    /// Тест 410 (Р.49 Б6): Cancel останавливает и текущую задачу, и очередь,
    /// а `is_running` говорит про РАБОТУ, а не про наличие окна — иначе
    /// перерисовка не остановится после финиша (повтор дефекта t38).
    #[test]
    fn t410_cancel_clears_everything_and_running_means_running() {
        let mut a = Analysis::default();
        assert!(!a.is_running(), "пустая подсистема считается работающей");
        let spin = |ctl: crate::bg::BgCtl, _s: crate::bg::BgSend<f32>| {
            while !ctl.should_stop() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err::<AnalysisOut, String>("cancelled".into())
        };
        a.request_with(AnalysisKind::Loudness(MediaId(1)), 1, false, spin);
        a.request_with(AnalysisKind::Loudness(MediaId(2)), 1, false, spin);

        // задача, которая крутится, пока её не отменят
        a.poll(spawn_default);
        assert!(a.is_running(), "запущенная задача не считается работающей");

        a.cancel_all();
        assert_eq!(a.queued(), 0, "очередь пережила отмену");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.is_running() && std::time::Instant::now() < deadline {
            a.poll(|_| done(Err("unused".into())));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!a.is_running(), "отменённая задача продолжает считаться работающей");
    }
}
