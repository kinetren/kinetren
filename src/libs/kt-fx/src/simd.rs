//! Рантайм-выбор SIMD-уровня CPU-ядер видеорендера (Р.32).
//!
//! Требование пользователя: «надо аккуратно как-то сделать, вдруг нету
//! супер-современных инструкций» — поэтому сборка остаётся baseline
//! x86-64 (SSE2), а AVX2-ядра включаются ТОЛЬКО после проверки в рантайме.
//! CPU без AVX2 получает ровно тот же скалярный код, что и раньше.
//!
//! ГЛАВНЫЙ инвариант (см. kt-fx/CLAUDE.md, инвариант №11 корневого):
//! SIMD-ядро обязано быть БАЙТ-В-БАЙТ равно скалярному на любом CPU —
//! CPU-путь остаётся эталоном детерминизма, golden'ы t26/t11/t67 не
//! перегенерируются.
//!
//! `KT_SIMD=0` — принудительный скаляр (отладка/сравнение, в ряду
//! KT_VIDEO_GPU/KT_AUTOPLAY).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimdLevel {
    Scalar,
    /// AVX2 (включает AVX и SSE4.1). Отдельного SSE-уровня нет намеренно:
    /// f64-ядрам нужны 4 лейна (__m256d), а 2-лейновый вариант не окупает
    /// вторую реализацию, которую пришлось бы держать бит-точной.
    Avx2,
}

/// Уровень считается ОДИН раз на процесс (как KT_VIDEO_GPU): ветка стоит
/// на вызов ядра (кадр/слой), а не на пиксель.
pub fn simd_level() -> SimdLevel {
    static LEVEL: std::sync::OnceLock<SimdLevel> = std::sync::OnceLock::new();
    *LEVEL.get_or_init(|| {
        if std::env::var("KT_SIMD").as_deref() == Ok("0") {
            return SimdLevel::Scalar;
        }
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            return SimdLevel::Avx2;
        }
        SimdLevel::Scalar
    })
}

/// Строка для перф-панели/диагностики.
pub fn simd_name() -> &'static str {
    match simd_level() {
        SimdLevel::Scalar => "scalar",
        SimdLevel::Avx2 => "avx2",
    }
}
