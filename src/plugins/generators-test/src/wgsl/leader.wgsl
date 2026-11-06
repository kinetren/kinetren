// a = the sweep's quarter, b = its remainder of 90 000, p0 = (centre, radius, bar): the cross, the
// ring and the sector growing clockwise from 12 o'clock, by signs of vector products.
fn covers(dx: i32, dy: i32) -> bool {
    var quad = 3u;
    if dx >= 0 && dy < 0 { quad = 0u; } else if dx >= 0 { quad = 1u; } else if dy >= 0 { quad = 2u; }
    if quad != u.a { return quad < u.a; }
    var p = vec2<i32>(-dy, -dx);
    if quad == 0u { p = vec2<i32>(dx, -dy); } else if quad == 1u { p = vec2<i32>(dy, dx); } else if quad == 2u { p = vec2<i32>(-dx, dy); }
    return p.x * 90000 >= i32(u.b) * max(p.x + p.y, 1);
}

fn pixel(x: u32, y: u32) -> u32 {
    let dx = i32(x) - i32(u.p0.x);
    let dy = i32(y) - i32(u.p0.y);
    let r = i32(u.p0.z);
    let bar = i32(u.p0.w);
    let d = i32(floor(sqrt(f32(dx * dx + dy * dy))));
    if abs(dx) <= bar || abs(dy) <= bar || abs(d - r) <= bar { return pack(vec4<f32>(220.0, 220.0, 220.0, 255.0)); }
    if d <= r && covers(dx, dy) { return pack(vec4<f32>(60.0, 60.0, 60.0, 255.0)); }
    return pack(vec4<f32>(16.0, 16.0, 16.0, 255.0));
}
