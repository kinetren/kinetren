// An arrow from a to b. p0 = colour, p1 = (a, b), p2 = (half thickness, head length, head half width, feather).
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let a = u.p1.xy;
    let b = u.p1.zw;
    let len = max(length(b - a), 1e-3);
    let dir = (b - a) / len;
    let head = min(u.p2.y, len);
    let base = b - dir * head;
    let shaft = sd_segment(q, a, base) - u.p2.x;
    // the head: a triangle base±normal·w, b — the max of its three half-planes
    let nrm = vec2<f32>(-dir.y, dir.x);
    let l = q - b;
    let along = -dot(l, dir) - head;
    let side1 = dot(l, normalize(nrm * head + dir * u.p2.z));
    let side2 = dot(l, normalize(-nrm * head + dir * u.p2.z));
    let tri = max(along, max(side1, side2));
    return shape(min(shaft, tri), u.p2.w);
}
