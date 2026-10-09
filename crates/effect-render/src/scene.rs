use crate::{
    geometry::{Vertex, density_color, stroke_into},
    gpu::{MeshVertex, Params},
};
use anyhow::Result;
use effect_model::equations::EquationBundle;
use effect_runtime::state::{FrameInput, PresetState};

pub struct Scene {
    pub state: PresetState,
    pub params: Params,
    pub mesh: Vec<MeshVertex>,
    pub vertices: Vec<Vertex>,
    pub batches: Vec<(std::ops::Range<u32>, bool)>,
    pub canvas: [u32; 2],
    pub adapted: bool,
    /// Explicit aggregated spectral-band adaptation; not isolated instruments.
    pub spectrum: [f32; 64],
}
impl Scene {
    pub fn new(bundle: EquationBundle, canvas: [u32; 2], adapted: bool) -> Result<Self> {
        Ok(Self {
            state: PresetState::new(bundle, 12345)?,
            params: Params::default(),
            mesh: Vec::with_capacity(49 * 33),
            vertices: Vec::with_capacity(65536),
            batches: vec![],
            canvas,
            adapted,
            spectrum: [0.0; 64],
        })
    }
    pub fn prepare(&mut self, input: FrameInput, waveform: &[[f32; 2]]) -> Result<()> {
        self.state.frame(input)?;
        self.mesh.clear();
        self.vertices.clear();
        self.batches.clear();
        let s = &mut self.state;
        for y in 0..=32 {
            for x in 0..=48 {
                let uv = [x as f64 / 48.0, y as f64 / 32.0];
                s.pixel(uv[0], uv[1])?;
                let zoom = s.pixel_value("zoom").abs().max(0.01);
                let sx = s.pixel_value("sx").abs().max(0.01);
                let sy = s.pixel_value("sy").abs().max(0.01);
                let cx = s.pixel_value("cx");
                let cy = s.pixel_value("cy");
                let rot = s.pixel_value("rot");
                let dx = (uv[0] - cx) / sx / zoom;
                let dy = (uv[1] - cy) / sy / zoom;
                let warp = s.pixel_value("warp") * 0.003;
                let sample = [
                    dx * rot.cos() - dy * rot.sin()
                        + cx
                        + s.pixel_value("dx")
                        + warp * (input.time + uv[1] * 6.0).sin(),
                    dx * rot.sin()
                        + dy * rot.cos()
                        + cy
                        + s.pixel_value("dy")
                        + warp * (input.time * 0.7 + uv[0] * 6.0).cos(),
                ];
                let original = [uv[0] as f32, uv[1] as f32];
                self.mesh.push(MeshVertex {
                    position: original,
                    original,
                    warped: [sample[0] as f32, sample[1] as f32],
                });
            }
        }
        self.params.data[0] = [
            input.time as f32,
            input.fps as f32,
            input.frame as f32,
            input.progress as f32,
        ];
        self.params.data[1] = [
            input.bass as f32,
            input.mid as f32,
            input.treb as f32,
            ((input.bass + input.mid + input.treb) / 3.0) as f32,
        ];
        self.params.data[2] = [
            input.bass_att as f32,
            input.mid_att as f32,
            input.treb_att as f32,
            ((input.bass_att + input.mid_att + input.treb_att) / 3.0) as f32,
        ];
        // Style units adapt old pixel-space shader offsets, while every pass
        // still rasterizes the actual native canvas. No texture is reduced.
        let pixel_step = if self.adapted { 3.0 } else { 1.0 };
        self.params.data[3] = [
            self.canvas[0] as f32 / pixel_step,
            self.canvas[1] as f32 / pixel_step,
            pixel_step / self.canvas[0] as f32,
            pixel_step / self.canvas[1] as f32,
        ];
        self.params.data[4] = [
            1.0,
            self.canvas[1] as f32 / self.canvas[0] as f32,
            1.0,
            self.canvas[0] as f32 / self.canvas[1] as f32,
        ];
        self.params.data[5] = std::array::from_fn(|i| {
            ((input.frame as f64 * 0.754877666 + (i + 1) as f64 * 0.569840296).fract()) as f32
        });
        self.params.data[6] = [0.27, 0.61, 0.83, 0.39];
        let amount = s.static_value("fShader", 0.0) as f32;
        for i in 0..3 {
            self.params.data[7][i] = 1.0 - amount
                + amount * (0.5 + 0.5 * (input.time as f32 * 0.17 + i as f32 * 2.094).sin());
        }
        self.params.data[16] = [
            s.frame_value("blur1min") as f32,
            s.frame_value("blur1max") as f32,
            s.frame_value("blur2min") as f32,
            s.frame_value("blur2max") as f32,
        ];
        self.params.data[17] = [
            s.frame_value("blur3min") as f32,
            s.frame_value("blur3max") as f32,
            s.frame_value("blur1_edge_darken") as f32,
            0.0,
        ];
        self.params.data[7][3] = s.frame_value("decay") as f32;
        self.params.data[20] = [
            self.canvas[0] as f32,
            self.canvas[1] as f32,
            0.0,
            if self.adapted { 1.0 } else { 0.0 },
        ];
        self.params.data[18] = [
            s.frame_value("gamma") as f32,
            s.frame_value("echo_zoom") as f32,
            s.frame_value("echo_alpha") as f32,
            s.frame_value("echo_orient") as f32,
        ];
        for i in 1..=32 {
            self.params.data[8 + (i - 1) / 4][(i - 1) % 4] = s.frame_value(&format!("q{i}")) as f32;
        }
        let minimum: f32 = if self.adapted { 3.0 } else { 1.0 };
        let aspect = self.canvas[1] as f32 / self.canvas[0] as f32;
        // Source shapes deposit into native feedback before source custom waves.
        for index in 0..4 {
            if s.static_value(&format!("shapecode_{index}_enabled"), 0.0) == 0.0 {
                continue;
            }
            let instances = s
                .static_value(&format!("shapecode_{index}_num_inst"), 1.0)
                .clamp(0.0, 1024.0) as usize;
            for instance in 0..instances {
                s.shape(index, instance)?;
                // Preserve every state/RNG/memory invocation, but sample dense
                // authored geometry uniformly across its original index range.
                let stride = if self.adapted {
                    instances.div_ceil(64).max(1)
                } else {
                    1
                };
                if instance % stride != 0 {
                    continue;
                }
                let start = self.vertices.len() as u32;
                let sides = s.shape_value(index, "sides").clamp(3.0, 100.0) as usize;
                let center = [
                    s.shape_value(index, "x") as f32,
                    s.shape_value(index, "y") as f32,
                ];
                let rad = s.shape_value(index, "rad") as f32;
                let angle = s.shape_value(index, "ang") as f32;
                let color = |suffix: &str| -> [f32; 4] {
                    std::array::from_fn(|i| {
                        s.shape_value(index, &format!("{}{suffix}", ["r", "g", "b", "a"][i])) as f32
                    })
                };
                let textured = s.shape_value(index, "textured") != 0.0;
                let tex_angle = s.shape_value(index, "tex_ang") as f32;
                let tex_zoom = (s.shape_value(index, "tex_zoom") as f32).abs().max(0.001);
                let tex_point = |n: usize| {
                    let a = tex_angle + std::f32::consts::TAU * n as f32 / sides as f32;
                    [
                        0.5 + a.cos() * 0.5 / tex_zoom,
                        0.5 + a.sin() * 0.5 / tex_zoom,
                        if textured { 1.0 } else { 0.0 },
                    ]
                };
                let inner = color("");
                let outer = color("2");
                let border: [f32; 4] = std::array::from_fn(|i| {
                    s.shape_value(index, ["border_r", "border_g", "border_b", "border_a"][i]) as f32
                });
                for side in 0..sides {
                    let point = |n: usize| {
                        let a = angle + std::f32::consts::TAU * n as f32 / sides as f32;
                        [
                            center[0] + rad * a.cos() * aspect,
                            center[1] + rad * a.sin(),
                        ]
                    };
                    let a = point(side);
                    let b = point(side + 1);
                    if a.iter()
                        .chain(&b)
                        .chain(&center)
                        .chain(&inner)
                        .chain(&outer)
                        .all(|v| v.is_finite())
                    {
                        self.vertices.extend_from_slice(&[
                            Vertex {
                                position: center,
                                color: inner,
                                texture: [0.5, 0.5, if textured { 1.0 } else { 0.0 }],
                            },
                            Vertex {
                                position: a,
                                color: outer,
                                texture: tex_point(side),
                            },
                            Vertex {
                                position: b,
                                color: outer,
                                texture: tex_point(side + 1),
                            },
                        ]);
                    }
                    if border[3] > 0.0 {
                        let width = minimum.max(if s.shape_value(index, "thick") != 0.0 {
                            minimum * 1.5
                        } else {
                            minimum
                        });
                        stroke_into(
                            &mut self.vertices,
                            a,
                            b,
                            density_color(border, width, false, self.adapted),
                            width,
                            self.canvas,
                        );
                    }
                }
                self.batches.push((
                    start..self.vertices.len() as u32,
                    s.shape_value(index, "additive") != 0.0,
                ));
            }
        }
        for index in 0..4 {
            if s.static_value(&format!("wavecode_{index}_enabled"), 0.0) == 0.0 {
                continue;
            }
            s.begin_wave(index)?;
            let start = self.vertices.len() as u32;
            let samples = s.wave_frame_value(index, "samples").clamp(0.0, 512.0) as usize;
            let scale = s.static_value(&format!("wavecode_{index}_scaling"), 1.0)
                * s.static_value("fWaveScale", 1.0);
            let spectrum_input = s.static_value(&format!("wavecode_{index}_bSpectrum"), 0.0) != 0.0;
            let mut previous = None;
            for point in 0..samples {
                let sample = point as f64 / samples.saturating_sub(1).max(1) as f64;
                let values = if spectrum_input {
                    [self.spectrum[point * 64 / samples.max(1)]; 2]
                } else if waveform.is_empty() {
                    [0.0; 2]
                } else {
                    waveform[point * waveform.len() / samples.max(1)]
                };
                s.wave_point(
                    index,
                    sample,
                    values[0] as f64 * scale,
                    values[1] as f64 * scale,
                )?;
                let position = [
                    s.wave_point_value(index, "x") as f32,
                    s.wave_point_value(index, "y") as f32,
                ];
                let color: [f32; 4] = std::array::from_fn(|i| {
                    s.wave_point_value(index, ["r", "g", "b", "a"][i]) as f32
                });
                let dots = s.static_value(&format!("wavecode_{index}_bUseDots"), 0.0) != 0.0;
                let color = density_color(color, minimum, dots, self.adapted);
                if dots {
                    let half = minimum * 0.5;
                    let d = [half / self.canvas[0] as f32, half / self.canvas[1] as f32];
                    let p = [
                        [position[0] - d[0], position[1] - d[1]],
                        [position[0] + d[0], position[1] - d[1]],
                        [position[0] - d[0], position[1] + d[1]],
                        [position[0] + d[0], position[1] + d[1]],
                    ];
                    if position.iter().chain(&color).all(|v| v.is_finite()) {
                        for i in [0, 1, 2, 2, 1, 3] {
                            self.vertices.push(Vertex {
                                position: p[i],
                                color,
                                texture: [0.0; 3],
                            });
                        }
                    }
                } else if let Some(previous) = previous {
                    stroke_into(
                        &mut self.vertices,
                        previous,
                        position,
                        color,
                        minimum,
                        self.canvas,
                    );
                }
                previous = Some(position);
            }
            self.batches.push((
                start..self.vertices.len() as u32,
                s.static_value(&format!("wavecode_{index}_bAdditive"), 0.0) != 0.0,
            ));
        }
        // Independently authored PoC geometry for the source waveform mode families.
        // Colors/positions/scale remain source controlled; exact engine formulas are not copied.
        if waveform.len() > 1 && s.frame_value("wave_a") > 0.0 {
            let start = self.vertices.len() as u32;
            let color = [
                s.frame_value("wave_r") as f32,
                s.frame_value("wave_g") as f32,
                s.frame_value("wave_b") as f32,
                s.frame_value("wave_a") as f32,
            ];
            let color = density_color(color, minimum, false, self.adapted);
            let scale = s.static_value("fWaveScale", 1.0) as f32;
            let mode = s.frame_value("wave_mode").round() as i32;
            let center = [
                s.frame_value("wave_x") as f32,
                s.frame_value("wave_y") as f32,
            ];
            let parameter = s.frame_value("wave_mystery") as f32;
            let channels = if mode == 7 { 2 } else { 1 };
            for channel in 0..channels {
                let mut previous = None;
                for i in 0..waveform.len() {
                    let sample = i as f32 / (waveform.len() - 1) as f32;
                    let left = waveform[i][0] * scale;
                    let right = waveform[(i + 16) % waveform.len()][1] * scale;
                    let position = match mode {
                        0 => {
                            let angle = sample * std::f32::consts::TAU;
                            let radius = (0.18 + parameter * 0.1 + left * 0.12).abs();
                            [
                                center[0] + radius * angle.cos() * aspect,
                                center[1] + radius * angle.sin(),
                            ]
                        }
                        1 => {
                            let angle = left * std::f32::consts::PI + input.time as f32;
                            let radius = (0.2 + right * 0.08 + parameter * 0.1).abs();
                            [
                                center[0] + radius * angle.cos() * aspect,
                                center[1] + radius * angle.sin(),
                            ]
                        }
                        2 | 3 => [center[0] + right * 0.4 * aspect, center[1] + left * 0.4],
                        5 => {
                            let angle = input.time as f32 * 0.3;
                            let x = left * right;
                            let y = (left * left - right * right) * 0.5;
                            [
                                center[0] + (x * angle.cos() - y * angle.sin()) * aspect,
                                center[1] + x * angle.sin() + y * angle.cos(),
                            ]
                        }
                        6..=8 => {
                            let angle = parameter * std::f32::consts::PI;
                            let along = (sample - 0.5) * 0.9;
                            let signal = if mode == 8 {
                                self.spectrum[i * 64 / waveform.len()] * scale
                            } else {
                                waveform[i][channel] * scale
                            };
                            let across = (center[0] - 0.5) * 0.5
                                + signal * 0.15
                                + if mode == 7 {
                                    (center[1] - 0.5) * (if channel == 0 { -0.5 } else { 0.5 })
                                } else {
                                    0.0
                                };
                            [
                                0.5 + along * angle.cos() - across * angle.sin() * aspect,
                                0.5 + along * angle.sin() + across * angle.cos(),
                            ]
                        }
                        _ => [sample, center[1] + left * 0.25],
                    };
                    if let Some(a) = previous {
                        stroke_into(&mut self.vertices, a, position, color, minimum, self.canvas);
                    }
                    previous = Some(position);
                }
            }
            self.batches.push((
                start..self.vertices.len() as u32,
                s.frame_value("wave_additive") != 0.0,
            ));
        }
        // Source borders are feedback geometry, including one-frame seeds.
        let mut inset = [0.0f32; 2];
        for prefix in ["ob", "ib"] {
            let size = (s.frame_value(&format!("{prefix}_size")) as f32).clamp(0.0, 0.5) * 0.5;
            let color: [f32; 4] = std::array::from_fn(|i| {
                s.frame_value(&format!("{prefix}_{}", ["r", "g", "b", "a"][i])) as f32
            });
            let thickness = [
                size.max(minimum / self.canvas[0] as f32),
                size.max(minimum / self.canvas[1] as f32),
            ];
            if size > 0.0 && color[3] > 0.0 {
                let start = self.vertices.len() as u32;
                let [x, y] = inset;
                let [tx, ty] = thickness;
                for [left, top, right, bottom] in [
                    [x, y, 1.0 - x, y + ty],
                    [x, 1.0 - y - ty, 1.0 - x, 1.0 - y],
                    [x, y + ty, x + tx, 1.0 - y - ty],
                    [1.0 - x - tx, y + ty, 1.0 - x, 1.0 - y - ty],
                ] {
                    if right <= left || bottom <= top {
                        continue;
                    }
                    let corners = [[left, top], [right, top], [left, bottom], [right, bottom]];
                    for i in [0, 1, 2, 2, 1, 3] {
                        self.vertices.push(Vertex {
                            position: corners[i],
                            color,
                            texture: [0.0; 3],
                        });
                    }
                }
                self.batches
                    .push((start..self.vertices.len() as u32, false));
            }
            for value in &mut inset {
                *value = (*value + size).min(0.5);
            }
        }
        Ok(())
    }
}
