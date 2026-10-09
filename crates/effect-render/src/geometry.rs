#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    /// Source texture UV and feedback modulation weight (zero for plain strokes).
    pub texture: [f32; 3],
}
pub fn density_color(mut color: [f32; 4], width: f32, point: bool, adapted: bool) -> [f32; 4] {
    if adapted {
        let footprint = if point { width * width } else { width };
        color[3] = color[3].clamp(0.0, 1.0) / footprint.max(1.0);
    }
    color
}
pub fn stroke(
    a: [f32; 2],
    b: [f32; 2],
    color: [f32; 4],
    width: f32,
    canvas: [u32; 2],
) -> Vec<Vertex> {
    let mut vertices = vec![];
    stroke_into(&mut vertices, a, b, color, width, canvas);
    vertices
}
pub fn stroke_into(
    vertices: &mut Vec<Vertex>,
    a: [f32; 2],
    b: [f32; 2],
    color: [f32; 4],
    width: f32,
    canvas: [u32; 2],
) {
    if canvas.contains(&0)
        || !width.is_finite()
        || width <= 0.0
        || !a.iter().chain(&b).chain(&color).all(|v| v.is_finite())
    {
        return;
    }
    let dx = (b[0] - a[0]) * canvas[0] as f32;
    let dy = (b[1] - a[1]) * canvas[1] as f32;
    let length = dx.hypot(dy);
    if length < 1e-6 {
        return;
    }
    let normal = [
        -dy / length * width * 0.5 / canvas[0] as f32,
        dx / length * width * 0.5 / canvas[1] as f32,
    ];
    let corners = [
        [a[0] + normal[0], a[1] + normal[1]],
        [a[0] - normal[0], a[1] - normal[1]],
        [b[0] + normal[0], b[1] + normal[1]],
        [b[0] - normal[0], b[1] - normal[1]],
    ];
    for i in [0, 1, 2, 2, 1, 3] {
        vertices.push(Vertex {
            position: corners[i],
            color,
            texture: [0.0; 3],
        });
    }
}
