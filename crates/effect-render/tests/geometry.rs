use effect_render::geometry::stroke;

#[test]
fn wider_strokes_preserve_deposited_feedback_energy() {
    let color = [0.2, 0.4, 0.8, 0.75];
    let line = effect_render::geometry::density_color(color, 3.0, false, true);
    let point = effect_render::geometry::density_color(color, 3.0, true, true);
    assert!((line[3] * 3.0 - color[3]).abs() < 1e-6);
    assert!((point[3] * 9.0 - color[3]).abs() < 1e-6);
    assert_eq!(
        effect_render::geometry::density_color(color, 3.0, false, false),
        color
    );
}
#[test]
fn adapted_stroke_has_substantial_native_4k_coverage() {
    let vertices = stroke([0.1, 0.5], [0.9, 0.5], [1.0; 4], 3.0, [3840, 2160]);
    assert_eq!(vertices.len(), 6);
    let min = vertices
        .iter()
        .map(|v| v.position[1])
        .fold(f32::INFINITY, f32::min);
    let max = vertices
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(((max - min) * 2160.0 - 3.0).abs() < 0.01);
}
#[test]
fn invalid_geometry_does_not_produce_nan_triangles() {
    assert!(stroke([f32::NAN, 0.0], [1.0, 1.0], [1.0; 4], 3.0, [3840, 2160]).is_empty());
    assert!(stroke([0.0, 0.0], [1.0, 1.0], [1.0; 4], 3.0, [0, 0]).is_empty());
}
