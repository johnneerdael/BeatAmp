use milk_compile::equations::compile_lines;
use milk_import::{corpus, parser};
use std::path::Path;

#[test]
fn all_frozen_equation_sections_compile_with_original_source_maps() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let c = corpus::verify(root).unwrap();
    let mut failures = vec![];
    let mut sections = 0;
    for entry in &c.presets {
        let p = parser::parse(&corpus::verify_entry(root, entry, "presets").unwrap()).unwrap();
        for (scope, lines) in &p.sections {
            if scope == "warp" || scope == "comp" {
                continue;
            }
            sections += 1;
            match compile_lines(lines) {
                Ok(program) => assert!(program.instructions.iter().all(|i| {
                    lines
                        .iter()
                        .any(|line| line.source_line == i.span.source_line)
                })),
                Err(e) => failures.push(format!("{} {scope}: {e}", entry.path)),
            }
        }
    }
    assert!(sections > 500);
    assert!(
        failures.is_empty(),
        "{} compilation failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
