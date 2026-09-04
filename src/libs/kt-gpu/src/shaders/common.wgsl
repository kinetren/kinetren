// Общая шапка всех пассов kt-gpu (подклеивается в начало каждого .wgsl).
//
// Все текстуры — Rgba8Uint: textureLoad отдаёт СЫРЫЕ байты 0..255 без
// unorm-нормализации (u8/255.0 в f32 теряет точность и ломает зеркала
// целочисленной CPU-арифметики). Фильтрации/сэмплеров нет — билинейка
// считается вручную, как bilinear_into (kt-fx/src/video.rs).
//
// Униформа одна на все пассы (64 байта, generic-слоты) — один bind group
// layout на весь движок; неиспользуемые текстуры биндятся заглушкой 1×1.

struct Params {
    u0: vec4<u32>,
    u1: vec4<u32>,
    f0: vec4<f32>,
    f1: vec4<f32>,
    // MVP-матрица 3D-пассов (Р.29, pan3d); остальные пассы её не читают.
    m: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var T0: texture_2d<u32>;
@group(0) @binding(2) var T1: texture_2d<u32>;
@group(0) @binding(3) var T2: texture_2d<u32>;

// Fullscreen-треугольник: (-1,-1) (-1,3) (3,-1) накрывает вьюпорт целиком.
@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(vi / 2u)) * 4.0 - 1.0;
    let y = f32(i32(vi & 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}
