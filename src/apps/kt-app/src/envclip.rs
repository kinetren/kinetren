//! Буфер обмена огибающих: СИСТЕМНЫЙ, без зеркала внутри программы (Р.61).
//!
//! **Претензия владельца, из которой всё следует.** «Судя по тому что кнопка
//! может быть "disabled" — в коде есть какое-то состояние буфера обмена —
//! что плохо, нужно по нажатию кнопки брать значение из реального буфера и
//! уже потом проверять можно вставить или нет, и предложить действия.»
//!
//! Он прав дважды. Копирование писало И в системный буфер, И в зеркало
//! `KtApp::env_clipboard`, а доступность кнопки решалась ПО ЗЕРКАЛУ — значит
//! скопированное во втором окне программы вставить было нельзя. Второй
//! половиной того же места кнопка гасла на чужом виде эффекта, то есть
//! сопоставление полей не отсутствовало, а было запрещено.
//!
//! **Почему чтение — В ФОНЕ, а не прямо в обработчике нажатия.** На X11
//! буфером ВЛАДЕЕТ окно. Если владелец — наше же окно (а это самый частый
//! случай: человек только что скопировал огибающую здесь), синхронное чтение
//! шлёт запрос выделения в наш собственный цикл событий — который в этот
//! момент выполняет отрисовку и ответить не может. Получилось бы зависание
//! ровно в том сценарии, ради которого фича и делается. Поэтому нажатие
//! ЗАКАЗЫВАЕТ чтение конечной отменяемой задачей (`BgJob`, инвариант 5), а
//! ответ приходит следующим кадром.

use crate::bg::BgJob;

/// Чем кончилось чтение буфера. Ошибка НАЗВАНА: «невозможность действия —
/// это ошибка, а не тишина» (правило 16 чек-листа).
pub enum ClipRead {
    /// Разобранный блок огибающей.
    Envelope(Box<kt_model::ops::EnvelopeBlock>),
    /// В буфере текст, но не наш блок.
    Foreign,
    /// Буфер пуст или недоступен (нет менеджера буфера, нет X-сервера).
    Empty,
}

/// Заказ на чтение: кому положить результат и сама задача.
pub struct ClipJob {
    /// Куда вставлять, когда придёт ответ: событие и слот цепочки.
    pub target: (kt_model::ClipId, usize),
    pub job: BgJob<(), ClipRead>,
}

/// Прочитать системный буфер и разобрать его.
///
/// Живёт отдельной функцией, потому что её зовут ДВА входа (окно эффектов и
/// окно морфера) и потому что её тело — единственное место, где программа
/// вообще трогает системный буфер на чтение.
pub fn read_clipboard() -> ClipRead {
    let text = match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(t) => t,
        Err(_) => return ClipRead::Empty,
    };
    if text.trim().is_empty() {
        return ClipRead::Empty;
    }
    match serde_json::from_str::<kt_model::ops::EnvelopeBlock>(&text) {
        Ok(b) => ClipRead::Envelope(Box::new(b)),
        Err(_) => ClipRead::Foreign,
    }
}

/// Заказать чтение буфера в фоне.
pub fn spawn_read(target: (kt_model::ClipId, usize)) -> ClipJob {
    ClipJob { target, job: BgJob::spawn("envclip", |_ctl, _send| read_clipboard()) }
}

/// Окно сопоставления «кривая → параметр цели» (Р.61).
///
/// Показывается ТОЛЬКО когда остаток непуст: у своего вида эффекта имена
/// совпадают целиком, и лишний шаг в самом частом случае был бы налогом.
pub struct FitDialog {
    pub target: (kt_model::ClipId, usize),
    /// Сдвиг ключей «вход к входу» — тот же, что у обычной вставки.
    pub delta: i64,
    /// Кривые, которым адрес выбирает человек; `None` — «не класть».
    pub rows: Vec<(kt_model::ParamCurve, Option<String>)>,
    /// Свободные параметры цели.
    pub free: Vec<String>,
    /// Что уже легло молча — применяется вместе с выбором, одним шагом.
    pub matched: Vec<kt_model::ParamCurve>,
    pub open: bool,
}

impl crate::app::KtApp {
    /// Забрать прочитанный буфер и решить его судьбу (Р.61).
    ///
    /// Три исхода, и ни одного молчаливого: наш блок — кладём (при чужом
    /// виде эффекта сперва спросив про остаток), чужой текст и пустой буфер
    /// — НАЗВАННАЯ ошибка в строке состояния.
    pub(crate) fn poll_env_clipboard(&mut self) {
        let Some(req) = self.env_read.as_mut() else { return };
        if !req.job.poll() {
            return;
        }
        let (target, done) = (req.target, req.job.done.take());
        self.env_read = None;
        let Some(read) = done else { return };
        let block = match read {
            ClipRead::Envelope(b) => *b,
            ClipRead::Foreign => {
                self.warn(crate::i18n::tr(crate::i18n::k::fx::PASTE_ENV_FOREIGN));
                return;
            }
            ClipRead::Empty => {
                self.warn(crate::i18n::tr(crate::i18n::k::fx::PASTE_ENV_EMPTY));
                return;
            }
        };
        let (id, slot) = target;
        let Some(clip) = self.model.clip(id).cloned() else { return };
        let Some(fx) = clip.fx.get(slot).cloned() else { return };
        // ключи живут в тиках МАТЕРИАЛА: переносим их на точку входа
        // этого события, иначе кривая уехала бы на чужой кусок
        let delta = clip.source_in.0 - block.src_in.0;
        let fit = kt_model::ops::fit_envelope(&block.curves, &fx);
        if fit.leftover.is_empty() {
            self.apply_env_fit(target, delta, fit.matched, Vec::new());
            return;
        }
        // остаток есть — спрашиваем; окно показывается ТОЛЬКО в этом случае
        self.env_fit = Some(FitDialog {
            target,
            delta,
            rows: fit.leftover.into_iter().map(|c| (c, None)).collect(),
            free: fit.free,
            matched: fit.matched,
            open: true,
        });
    }

    /// Положить кривые в слот одним шагом истории.
    pub(crate) fn apply_env_fit(
        &mut self,
        target: (kt_model::ClipId, usize),
        delta: i64,
        matched: Vec<kt_model::ParamCurve>,
        remapped: Vec<kt_model::ParamCurve>,
    ) {
        let (id, slot) = target;
        let Some(clip) = self.model.clip(id).cloned() else { return };
        let mut auto = clip.fx_auto.clone();
        if auto.len() <= slot {
            auto.resize(slot + 1, Vec::new());
        }
        for mut c in matched.into_iter().chain(remapped) {
            for kf in &mut c.keys {
                kf.t = kt_time::Tick((kf.t.0 + delta).max(0));
            }
            auto[slot].retain(|old| old.param != c.param);
            auto[slot].push(c);
        }
        self.gesture_counter += 1;
        let g = self.gesture_counter;
        self.set_fx_auto_coalesced(id, auto, g);
    }
}

impl crate::app::KtApp {
    /// Окно сопоставления «кривая → параметр цели».
    ///
    /// Показывается ТОЛЬКО при непустом остатке: у своего вида эффекта имена
    /// совпадают целиком, и лишний шаг в самом частом случае был бы налогом.
    /// Отказ от выбора кривую НЕ теряет молча — их число называется.
    pub(crate) fn draw_env_fit(&mut self, ctx: &eframe::egui::Context) {
        use crate::i18n::{k, tr};
        use eframe::egui;
        let Some(mut d) = self.env_fit.take() else { return };
        let mut go = false;
        let mut cancel = false;
        let mut open = d.open;
        egui::Window::new(tr(k::fx::FIT_TITLE))
            .id(egui::Id::new(("env_fit", d.target.0.0, d.target.1)))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(tr(k::fx::FIT_HINT)).weak().small());
                egui::Grid::new("env_fit_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    for (i, (c, pick)) in d.rows.iter_mut().enumerate() {
                        ui.label(crate::widgets::param_label(&c.param));
                        let text = pick.clone().unwrap_or_else(|| tr(k::fx::FIT_SKIP).into());
                        let r = egui::ComboBox::from_id_salt(("env_fit", i))
                            .selected_text(text)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(pick.is_none(), tr(k::fx::FIT_SKIP))
                                    .clicked()
                                {
                                    *pick = None;
                                }
                                for f in &d.free {
                                    let on = pick.as_deref() == Some(f.as_str());
                                    if ui
                                        .selectable_label(on, crate::widgets::param_label(f))
                                        .clicked()
                                    {
                                        *pick = Some(f.clone());
                                    }
                                }
                            });
                        crate::widgets::field_probe::note(
                            &format!("envfit.pick.{i}"),
                            r.response.rect,
                        );
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let b = ui.button(tr(k::fx::FIT_DO));
                    crate::widgets::field_probe::note("envfit.ok", b.rect);
                    if b.clicked() {
                        go = true;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if go {
            let mut remapped = Vec::new();
            let mut skipped = 0usize;
            for (c, pick) in d.rows.drain(..) {
                match pick {
                    Some(to) => remapped.push(kt_model::ops::remap_curve(c, &to)),
                    None => skipped += 1,
                }
            }
            let matched = std::mem::take(&mut d.matched);
            self.apply_env_fit(d.target, d.delta, matched, remapped);
            // пропущенное НАЗЫВАЕТСЯ: кривая не вправе исчезнуть молча
            if skipped > 0 {
                self.warn(&crate::tf!(k::fx::FIT_LOST, skipped));
            }
            return;
        }
        if cancel || !open {
            return;
        }
        d.open = open;
        self.env_fit = Some(d);
    }
}
