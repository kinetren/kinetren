//! Правый клик В ПАНЕЛЯХ — второй реестр целей (Р.49, п.363).
//!
//! **Почему реестр ВТОРОЙ, а не расширение `CtxTarget`.** У целей таймлайна
//! общая сцена (одна полоса, один хит-тест по позиции, одно меню в конце
//! кадра) и общий род идентификатора — время и дорожка. У панелей ничего из
//! этого нет: слот цепочки живёт в окне FX, карточка — в бине, фейдер — в
//! метрах; каждый рисуется своим кодом и знает свою цель сам. Свалить их в
//! один enum значило бы получить `match`, половина веток которого никогда не
//! встречается в своём месте.
//!
//! **Зачем реестр вообще.** Разведка нашла двенадцать мест, где объект
//! отвечает на НАВЕДЕНИЕ подсказкой, а на правую кнопку — ничем. Пользователь
//! учится один раз: если правая кнопка где-то показала меню, он жмёт её
//! везде. Молчание читается как «объекта нет» — ровно тот класс тупиков, что
//! стоил раундов Р.33-Р.40.
//!
//! Тип `PanelEntity` существует ради КОМПИЛЯТОРА: литерал `ALL` фиксированной
//! длины ломает сборку, пока новую панельную сущность не назвали, а тест 395 —
//! исчерпывающий `match` по глаголам.

use kt_model::ClipId;

/// Объект панели, по которому нажали правой кнопкой.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelTarget {
    /// Слот цепочки эффектов: (клип-владелец цепочки, индекс слота).
    FxSlot { chain: ClipId, slot: usize },
    /// Приколотая голова цепочки (источник морфер-слоя).
    FxHead { chain: ClipId },
    /// Ключ кривой автоматизации: (цепочка, слот, кривая, ключ).
    CurveKey { chain: ClipId, slot: usize, curve: usize, key: usize },
    /// Свободный фон медиа-бина.
    BinEmpty,
    /// Окно превью.
    Preview,
    /// Панель метров (шкалы или фейдер).
    Meters,
}

/// Род панельного объекта — по одному на каждую «вещь, которой управляют».
// Тип живёт ради КОМПИЛЯТОРА и читается тестом 395; в продуктовом пути его
// не зовут, и это норма.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum PanelEntity {
    FxSlot = 0,
    FxHead,
    CurveKey,
    Bin,
    Preview,
    Meters,
}

#[allow(dead_code)]
impl PanelEntity {
    pub const N: usize = PanelEntity::Meters as usize + 1;
    /// Длина задана `N`: добавил вариант — литерал перестал соответствовать
    /// длине, и не собирается ВСЁ приложение, а не только тесты.
    pub const ALL: [PanelEntity; Self::N] = [
        PanelEntity::FxSlot,
        PanelEntity::FxHead,
        PanelEntity::CurveKey,
        PanelEntity::Bin,
        PanelEntity::Preview,
        PanelEntity::Meters,
    ];
}

impl PanelTarget {
    /// Какому роду принадлежит цель. Match исчерпывающий: завёл новую цель —
    /// обязан назвать её род.
    #[allow(dead_code)]
    pub fn entity(self) -> PanelEntity {
        match self {
            PanelTarget::FxSlot { .. } => PanelEntity::FxSlot,
            PanelTarget::FxHead { .. } => PanelEntity::FxHead,
            PanelTarget::CurveKey { .. } => PanelEntity::CurveKey,
            PanelTarget::BinEmpty => PanelEntity::Bin,
            PanelTarget::Preview => PanelEntity::Preview,
            PanelTarget::Meters => PanelEntity::Meters,
        }
    }
}

impl crate::app::KtApp {
    /// Запомнить, по чему нажали правой кнопкой в панели.
    ///
    /// Цели ГАСЯТ ДРУГ ДРУГА: меню на экране одно, и если бы протухшая цель
    /// таймлайна пережила правый клик в панели, тест прочитал бы её и
    /// зазеленел на сломанном (а код, спрашивающий «а что там выделено»,
    /// ответил бы про вчерашний объект).
    pub fn set_panel_target(&mut self, t: PanelTarget) {
        self.ctx_panel = Some(t);
        self.ctx_target = None;
    }
}
