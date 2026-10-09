use crate::vm::{Memory, Random, Vm};
use anyhow::{Result, ensure};
use effect_model::equations::*;
use std::collections::BTreeMap;

const FRAME_BUDGET: u64 = 4_000_000;
const INIT_BUDGET: u64 = 8_000_000;
const INVOCATION_BUDGET: u64 = 1_000_000;
const Q_FIELDS: [&str; 32] = [
    "q1", "q2", "q3", "q4", "q5", "q6", "q7", "q8", "q9", "q10", "q11", "q12", "q13", "q14", "q15",
    "q16", "q17", "q18", "q19", "q20", "q21", "q22", "q23", "q24", "q25", "q26", "q27", "q28",
    "q29", "q30", "q31", "q32",
];
const T_FIELDS: [&str; 8] = ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"];
const WARP_FIELDS: [&str; 10] = [
    "zoom", "zoomexp", "rot", "warp", "cx", "cy", "dx", "dy", "sx", "sy",
];

#[derive(Clone, Copy)]
pub struct FrameInput {
    pub frame: u64,
    pub time: f64,
    pub fps: f64,
    pub progress: f64,
    pub bass: f64,
    pub mid: f64,
    pub treb: f64,
    pub bass_att: f64,
    pub mid_att: f64,
    pub treb_att: f64,
    pub width: u32,
    pub height: u32,
    pub mesh_x: u32,
    pub mesh_y: u32,
}
impl Default for FrameInput {
    fn default() -> Self {
        Self {
            frame: 0,
            time: 0.0,
            fps: 30.0,
            progress: 0.0,
            bass: 1.0,
            mid: 1.0,
            treb: 1.0,
            bass_att: 1.0,
            mid_att: 1.0,
            treb_att: 1.0,
            width: 3840,
            height: 2160,
            mesh_x: 48,
            mesh_y: 32,
        }
    }
}
struct CustomWave {
    frame: Vm,
    point: Vm,
    initial_t: [f64; 8],
    defaults: Vec<(String, f64)>,
}
struct CustomShape {
    frame: Vm,
    initial_t: [f64; 8],
    defaults: Vec<(String, f64)>,
}

/// State owner for one preset. Separate main/mesh/custom environments preserve
/// ordinary variables; only the documented q/t and host bindings cross scopes.
pub struct PresetState {
    programs: BTreeMap<String, Program>,
    static_values: BTreeMap<String, f64>,
    frame_vm: Vm,
    pixel_vm: Vm,
    waves: Vec<CustomWave>,
    shapes: Vec<CustomShape>,
    initial_q: [f64; 32],
    frame_q: [f64; 32],
    monitor: f64,
    defaults: Vec<(String, f64)>,
    global: Memory,
    random: Random,
    input: FrameInput,
    spent: u64,
}
impl PresetState {
    pub fn new(bundle: EquationBundle, seed: u32) -> Result<Self> {
        ensure!(
            bundle.abi == EQUATION_ABI,
            "unsupported equation bundle ABI"
        );
        let zero = Program {
            abi: bundle.abi,
            symbols: bundle.symbols.clone(),
            instructions: vec![Instruction {
                op: Op::Push(0.0),
                span: Span {
                    source_line: 1,
                    column: 1,
                },
            }],
        };
        let mut programs = BTreeMap::new();
        for (scope, instructions) in bundle.sections {
            let program = Program {
                abi: bundle.abi,
                symbols: bundle.symbols.clone(),
                instructions,
            };
            Vm::new(&program)?;
            programs.insert(scope, program);
        }
        let mut s = Self {
            programs,
            static_values: bundle.static_values,
            frame_vm: Vm::new(&zero)?,
            pixel_vm: Vm::new(&zero)?,
            waves: vec![],
            shapes: vec![],
            initial_q: [0.0; 32],
            frame_q: [0.0; 32],
            monitor: 0.0,
            defaults: vec![],
            global: Memory::default(),
            random: Random::new(seed),
            input: FrameInput::default(),
            spent: 0,
        };
        s.defaults = main_defaults(&s.static_values);
        reset(&mut s.frame_vm, &s.defaults)?;
        bind(&mut s.frame_vm, s.input)?;
        execute(
            &s.programs,
            "per_frame_init",
            &mut s.frame_vm,
            &mut s.global,
            &mut s.random,
            &mut s.spent,
            INIT_BUDGET,
        )?;
        s.initial_q = q_values(&s.frame_vm);
        s.monitor = s.frame_vm.get("monitor");
        for index in 0..4 {
            let mut wave = CustomWave {
                frame: Vm::new(&zero)?,
                point: Vm::new(&zero)?,
                initial_t: [0.0; 8],
                defaults: custom_defaults(&s.static_values, "wave", index),
            };
            reset(&mut wave.frame, &wave.defaults)?;
            bind(&mut wave.frame, s.input)?;
            set_q(&mut wave.frame, &s.initial_q)?;
            execute(
                &s.programs,
                &format!("wave_{index}_init"),
                &mut wave.frame,
                &mut s.global,
                &mut s.random,
                &mut s.spent,
                INIT_BUDGET,
            )?;
            wave.initial_t = t_values(&wave.frame);
            s.waves.push(wave);
            let mut shape = CustomShape {
                frame: Vm::new(&zero)?,
                initial_t: [0.0; 8],
                defaults: custom_defaults(&s.static_values, "shape", index),
            };
            reset(&mut shape.frame, &shape.defaults)?;
            bind(&mut shape.frame, s.input)?;
            set_q(&mut shape.frame, &s.initial_q)?;
            execute(
                &s.programs,
                &format!("shape_{index}_init"),
                &mut shape.frame,
                &mut s.global,
                &mut s.random,
                &mut s.spent,
                INIT_BUDGET,
            )?;
            shape.initial_t = t_values(&shape.frame);
            s.shapes.push(shape);
        }
        Ok(s)
    }
    pub fn frame(&mut self, input: FrameInput) -> Result<()> {
        ensure!(
            input.width > 0 && input.height > 0 && input.fps > 0.0,
            "invalid frame dimensions/timing"
        );
        self.input = input;
        self.spent = 0;
        reset(&mut self.frame_vm, &self.defaults)?;
        bind(&mut self.frame_vm, input)?;
        set_q(&mut self.frame_vm, &self.initial_q)?;
        self.frame_vm.set("monitor", self.monitor)?;
        execute(
            &self.programs,
            "per_frame",
            &mut self.frame_vm,
            &mut self.global,
            &mut self.random,
            &mut self.spent,
            FRAME_BUDGET,
        )?;
        self.monitor = self.frame_vm.get("monitor");
        self.frame_q = q_values(&self.frame_vm);
        bind(&mut self.pixel_vm, input)?;
        set_q(&mut self.pixel_vm, &self.frame_q)?;
        Ok(())
    }
    pub fn frame_value(&self, name: &str) -> f64 {
        self.frame_vm.get(name)
    }
    pub fn pixel(&mut self, x: f64, y: f64) -> Result<()> {
        for name in WARP_FIELDS {
            self.pixel_vm.set(name, self.frame_vm.get(name))?;
        }
        self.pixel_vm.set("x", x)?;
        self.pixel_vm.set("y", y)?;
        let dx = (x - 0.5) * 2.0;
        let dy = (y - 0.5) * 2.0;
        self.pixel_vm.set("rad", dx.hypot(dy))?;
        self.pixel_vm.set("ang", dy.atan2(dx))?;
        execute(
            &self.programs,
            "per_pixel",
            &mut self.pixel_vm,
            &mut self.global,
            &mut self.random,
            &mut self.spent,
            FRAME_BUDGET,
        )
    }
    pub fn pixel_value(&self, name: &str) -> f64 {
        self.pixel_vm.get(name)
    }
    pub fn begin_wave(&mut self, index: usize) -> Result<()> {
        let q = self.frame_q;
        let w = self
            .waves
            .get_mut(index)
            .ok_or_else(|| anyhow::anyhow!("custom wave index exceeds 4"))?;
        reset(&mut w.frame, &w.defaults)?;
        bind(&mut w.frame, self.input)?;
        set_q(&mut w.frame, &q)?;
        set_t(&mut w.frame, &w.initial_t)?;
        execute(
            &self.programs,
            &format!("wave_{index}_per_frame"),
            &mut w.frame,
            &mut self.global,
            &mut self.random,
            &mut self.spent,
            FRAME_BUDGET,
        )?;
        bind(&mut w.point, self.input)?;
        set_q(&mut w.point, &q_values(&w.frame))?;
        set_t(&mut w.point, &t_values(&w.frame))?;
        Ok(())
    }
    pub fn wave_point(
        &mut self,
        index: usize,
        sample: f64,
        value1: f64,
        value2: f64,
    ) -> Result<()> {
        let w = self
            .waves
            .get_mut(index)
            .ok_or_else(|| anyhow::anyhow!("custom wave index exceeds 4"))?;
        for (name, value) in [
            ("sample", sample),
            ("value1", value1),
            ("value2", value2),
            ("x", 0.5 + value1),
            ("y", 0.5 + value2),
        ] {
            w.point.set(name, value)?;
        }
        for name in ["r", "g", "b", "a"] {
            w.point.set(name, w.frame.get(name))?;
        }
        execute(
            &self.programs,
            &format!("wave_{index}_per_point"),
            &mut w.point,
            &mut self.global,
            &mut self.random,
            &mut self.spent,
            FRAME_BUDGET,
        )
    }
    pub fn wave_frame_value(&self, index: usize, name: &str) -> f64 {
        self.waves.get(index).map_or(0.0, |w| w.frame.get(name))
    }
    pub fn wave_point_value(&self, index: usize, name: &str) -> f64 {
        self.waves.get(index).map_or(0.0, |w| w.point.get(name))
    }
    pub fn shape(&mut self, index: usize, instance: usize) -> Result<()> {
        ensure!(instance < 1024, "custom shape instance budget exceeded");
        let q = self.frame_q;
        let s = self
            .shapes
            .get_mut(index)
            .ok_or_else(|| anyhow::anyhow!("custom shape index exceeds 4"))?;
        reset(&mut s.frame, &s.defaults)?;
        bind(&mut s.frame, self.input)?;
        set_q(&mut s.frame, &q)?;
        set_t(&mut s.frame, &s.initial_t)?;
        s.frame.set("instance", instance as f64)?;
        execute(
            &self.programs,
            &format!("shape_{index}_per_frame"),
            &mut s.frame,
            &mut self.global,
            &mut self.random,
            &mut self.spent,
            FRAME_BUDGET,
        )
    }
    pub fn shape_value(&self, index: usize, name: &str) -> f64 {
        self.shapes.get(index).map_or(0.0, |s| s.frame.get(name))
    }
    pub fn static_value(&self, name: &str, default: f64) -> f64 {
        *self.static_values.get(name).unwrap_or(&default)
    }
    pub fn instruction_count(&self) -> u64 {
        self.spent
    }
    pub fn memory_bytes(&self) -> usize {
        self.global.allocated_bytes()
            + self.frame_vm.local_memory_bytes()
            + self.pixel_vm.local_memory_bytes()
            + self
                .waves
                .iter()
                .map(|w| w.frame.local_memory_bytes() + w.point.local_memory_bytes())
                .sum::<usize>()
            + self
                .shapes
                .iter()
                .map(|s| s.frame.local_memory_bytes())
                .sum::<usize>()
    }
    pub fn nonfinite_assignments(&self) -> u64 {
        self.frame_vm.nonfinite_assignments
            + self.pixel_vm.nonfinite_assignments
            + self
                .waves
                .iter()
                .map(|w| w.frame.nonfinite_assignments + w.point.nonfinite_assignments)
                .sum::<u64>()
            + self
                .shapes
                .iter()
                .map(|s| s.frame.nonfinite_assignments)
                .sum::<u64>()
    }
}
fn execute(
    programs: &BTreeMap<String, Program>,
    scope: &str,
    vm: &mut Vm,
    global: &mut Memory,
    random: &mut Random,
    spent: &mut u64,
    limit: u64,
) -> Result<()> {
    if let Some(program) = programs.get(scope) {
        let remaining = limit.saturating_sub(*spent);
        let invocation_limit = if limit == INIT_BUDGET {
            INIT_BUDGET
        } else {
            INVOCATION_BUDGET
        };
        let result =
            vm.execute_prevalidated(program, global, random, remaining.min(invocation_limit));
        *spent += vm.instructions_executed;
        result.map_err(|e| anyhow::anyhow!("{scope}: {e}"))?;
    }
    Ok(())
}
fn reset(vm: &mut Vm, defaults: &[(String, f64)]) -> Result<()> {
    for (name, value) in defaults {
        vm.set(name, *value)?;
    }
    Ok(())
}
fn q_values(vm: &Vm) -> [f64; 32] {
    std::array::from_fn(|i| vm.get(Q_FIELDS[i]))
}
fn t_values(vm: &Vm) -> [f64; 8] {
    std::array::from_fn(|i| vm.get(T_FIELDS[i]))
}
fn set_q(vm: &mut Vm, q: &[f64; 32]) -> Result<()> {
    for (i, v) in q.iter().enumerate() {
        vm.set(Q_FIELDS[i], *v)?;
    }
    Ok(())
}
fn set_t(vm: &mut Vm, t: &[f64; 8]) -> Result<()> {
    for (i, v) in t.iter().enumerate() {
        vm.set(T_FIELDS[i], *v)?;
    }
    Ok(())
}
fn bind(vm: &mut Vm, i: FrameInput) -> Result<()> {
    let aspect = i.width as f64 / i.height as f64;
    for (name, v) in [
        ("frame", i.frame as f64),
        ("time", i.time),
        ("fps", i.fps),
        ("progress", i.progress),
        ("bass", i.bass),
        ("mid", i.mid),
        ("treb", i.treb),
        ("bass_att", i.bass_att),
        ("mid_att", i.mid_att),
        ("treb_att", i.treb_att),
        ("pixelsx", i.width as f64),
        ("pixelsy", i.height as f64),
        ("meshx", i.mesh_x as f64),
        ("meshy", i.mesh_y as f64),
        ("aspectx", if aspect >= 1.0 { 1.0 } else { aspect }),
        ("aspecty", if aspect >= 1.0 { 1.0 / aspect } else { 1.0 }),
    ] {
        vm.set(name, v)?;
    }
    Ok(())
}
fn main_defaults(values: &BTreeMap<String, f64>) -> Vec<(String, f64)> {
    let mut result = vec![];
    for name in WARP_FIELDS {
        let source = if name == "zoomexp" {
            "fZoomExponent"
        } else {
            name
        };
        let default = match name {
            "zoom" | "zoomexp" | "sx" | "sy" => 1.0,
            "cx" | "cy" => 0.5,
            _ => 0.0,
        };
        result.push((name.into(), *values.get(source).unwrap_or(&default)));
    }
    for name in [
        "wave_r", "wave_g", "wave_b", "wave_x", "wave_y", "ob_size", "ob_r", "ob_g", "ob_b",
        "ob_a", "ib_size", "ib_r", "ib_g", "ib_b", "ib_a", "mv_dx", "mv_dy", "mv_l", "mv_r",
        "mv_g", "mv_b", "mv_a",
    ] {
        result.push((name.into(), *values.get(name).unwrap_or(&0.0)));
    }
    for (name, key, default) in [
        ("decay", "fDecay", 0.98),
        ("wave_a", "fWaveAlpha", 1.0),
        ("wave_mode", "nWaveMode", 0.0),
        ("wave_mystery", "fWaveParam", 0.0),
        ("gamma", "fGammaAdj", 1.0),
        ("echo_zoom", "fVideoEchoZoom", 1.0),
        ("echo_alpha", "fVideoEchoAlpha", 0.0),
        ("echo_orient", "nVideoEchoOrientation", 0.0),
        ("wave_usedots", "bWaveDots", 0.0),
        ("wave_thick", "bWaveThick", 0.0),
        ("wave_additive", "bAdditiveWaves", 0.0),
        ("wave_brighten", "bMaximizeWaveColor", 0.0),
        ("darken_center", "bDarkenCenter", 0.0),
        ("wrap", "bTexWrap", 1.0),
        ("invert", "bInvert", 0.0),
        ("brighten", "bBrighten", 0.0),
        ("darken", "bDarken", 0.0),
        ("solarize", "bSolarize", 0.0),
        ("mv_x", "nMotionVectorsX", 0.0),
        ("mv_y", "nMotionVectorsY", 0.0),
        ("blur1min", "b1n", 0.0),
        ("blur1max", "b1x", 1.0),
        ("blur2min", "b2n", 0.0),
        ("blur2max", "b2x", 1.0),
        ("blur3min", "b3n", 0.0),
        ("blur3max", "b3x", 1.0),
        ("blur1_edge_darken", "b1ed", 0.0),
    ] {
        result.push((name.into(), *values.get(key).unwrap_or(&default)));
    }
    result
}
fn custom_defaults(values: &BTreeMap<String, f64>, kind: &str, index: usize) -> Vec<(String, f64)> {
    let fields: &[(&str, &str, f64)] = if kind == "wave" {
        &[
            ("r", "r", 1.0),
            ("g", "g", 1.0),
            ("b", "b", 1.0),
            ("a", "a", 1.0),
            ("samples", "samples", 512.0),
        ]
    } else {
        &[
            ("x", "x", 0.5),
            ("y", "y", 0.5),
            ("rad", "rad", 0.1),
            ("ang", "ang", 0.0),
            ("r", "r", 1.0),
            ("g", "g", 1.0),
            ("b", "b", 1.0),
            ("a", "a", 1.0),
            ("r2", "r2", 1.0),
            ("g2", "g2", 1.0),
            ("b2", "b2", 1.0),
            ("a2", "a2", 0.0),
            ("border_r", "border_r", 1.0),
            ("border_g", "border_g", 1.0),
            ("border_b", "border_b", 1.0),
            ("border_a", "border_a", 0.0),
            ("sides", "sides", 4.0),
            ("instances", "num_inst", 1.0),
            ("additive", "additive", 0.0),
            ("textured", "textured", 0.0),
            ("thick", "thickOutline", 0.0),
            ("tex_zoom", "tex_zoom", 1.0),
            ("tex_ang", "tex_ang", 0.0),
        ]
    };
    fields
        .iter()
        .map(|(name, key, default)| {
            (
                name.to_string(),
                *values
                    .get(&format!("{kind}code_{index}_{key}"))
                    .unwrap_or(default),
            )
        })
        .collect()
}
