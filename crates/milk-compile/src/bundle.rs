use anyhow::{Result, ensure};
use effect_model::equations::*;
use milk_import::parser::Preset;
use std::collections::{BTreeMap, BTreeSet};

/// Compile all equation scopes to one symbol layout, retaining ordered programs.
/// HLSL bodies remain a separate compiler route; this is not a playable package.
pub fn compile_equations(preset: &Preset) -> Result<EquationBundle> {
    let mut programs = BTreeMap::new();
    let mut symbols = BTreeSet::new();
    for (scope, lines) in &preset.sections {
        if matches!(scope.as_str(), "warp" | "comp") {
            continue;
        }
        let program =
            crate::equations::compile_lines(lines).map_err(|e| anyhow::anyhow!("{scope}: {e}"))?;
        symbols.extend(program.symbols.iter().cloned());
        programs.insert(scope.clone(), program);
    }
    for name in [
        "zoom",
        "zoomexp",
        "rot",
        "warp",
        "cx",
        "cy",
        "dx",
        "dy",
        "sx",
        "sy",
        "decay",
        "time",
        "fps",
        "frame",
        "progress",
        "bass",
        "mid",
        "treb",
        "bass_att",
        "mid_att",
        "treb_att",
        "x",
        "y",
        "rad",
        "ang",
        "sample",
        "value1",
        "value2",
        "r",
        "g",
        "b",
        "a",
        "r2",
        "g2",
        "b2",
        "a2",
        "samples",
        "sides",
        "instance",
        "instances",
        "additive",
        "textured",
        "thick",
        "tex_zoom",
        "tex_ang",
        "border_r",
        "border_g",
        "border_b",
        "border_a",
        "monitor",
        "pixelsx",
        "pixelsy",
        "aspectx",
        "aspecty",
        "meshx",
        "meshy",
        "gamma",
        "echo_zoom",
        "echo_alpha",
        "echo_orient",
        "wave_a",
        "wave_r",
        "wave_g",
        "wave_b",
        "wave_x",
        "wave_y",
        "wave_mode",
        "wave_mystery",
        "wave_usedots",
        "wave_thick",
        "wave_additive",
        "wave_brighten",
        "darken_center",
        "wrap",
        "invert",
        "brighten",
        "darken",
        "solarize",
        "ob_size",
        "ob_r",
        "ob_g",
        "ob_b",
        "ob_a",
        "ib_size",
        "ib_r",
        "ib_g",
        "ib_b",
        "ib_a",
        "mv_x",
        "mv_y",
        "mv_dx",
        "mv_dy",
        "mv_l",
        "mv_r",
        "mv_g",
        "mv_b",
        "mv_a",
        "blur1min",
        "blur1max",
        "blur2min",
        "blur2max",
        "blur3min",
        "blur3max",
        "blur1_edge_darken",
    ] {
        symbols.insert(name.to_owned());
    }
    for i in 1..=32 {
        symbols.insert(format!("q{i}"));
    }
    for i in 1..=8 {
        symbols.insert(format!("t{i}"));
    }
    ensure!(symbols.len() <= MAX_SYMBOLS, "bundle symbol limit exceeded");
    let symbols: Vec<String> = symbols.into_iter().collect();
    let ids: BTreeMap<&str, u16> = symbols
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u16))
        .collect();
    let mut sections = BTreeMap::new();
    for (scope, mut program) in programs {
        for instruction in &mut program.instructions {
            match &mut instruction.op {
                Op::Load(slot) | Op::Store(slot) => {
                    *slot = ids[program.symbols[*slot as usize].as_str()]
                }
                _ => {}
            }
        }
        sections.insert(scope, program.instructions);
    }
    Ok(EquationBundle {
        abi: EQUATION_ABI,
        symbols,
        sections,
        static_values: preset.numeric.clone(),
    })
}
