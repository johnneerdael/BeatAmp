use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use milk_import::{corpus, parser};
use std::path::PathBuf;
mod beat;
mod beat_adapt;
mod comparison;
mod equations;
mod render;

#[derive(Parser)]
#[command(about = "Inspect and adapt the frozen MilkDrop corpus")]
struct Args {
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Generate the fixed100 native-scene pilot; absent predictions stay pending.
    BeatAdaptCorpus {
        #[arg(long)]
        audit: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Render a standalone .beat at native 4K, with optional interleaved PCM16LE.
    BeatRender {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        pcm: Option<PathBuf>,
        #[arg(long, default_value_t = 48000)]
        rate: u32,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, default_value_t = 120)]
        frames: u32,
        #[arg(long)]
        output: PathBuf,
    },
    /// Generate a separate pilot from a read-only, hash-verified prediction audit.
    BeatAdaptReady {
        #[arg(long)]
        audit: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    BeatExport {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    BeatInspect {
        #[arg(long)]
        input: PathBuf,
    },
    PreparePlaylist {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 44100)]
        rate: u32,
    },
    PrepareComparison {
        #[arg(long)]
        input: PathBuf,
    },
    PackagePoc,
    RenderPoc {
        #[arg(long)]
        rank: u32,
        #[arg(long, default_value_t = 180)]
        frames: u32,
        #[arg(long)]
        output: PathBuf,
    },
    RenderFixed {
        #[arg(long)]
        rank: u32,
        #[arg(long, default_value_t = 180)]
        frames: u32,
        #[arg(long)]
        output: PathBuf,
    },
    ValidateShader {
        #[arg(long)]
        spirv: PathBuf,
        #[arg(long)]
        wgsl: bool,
        #[arg(long)]
        output: PathBuf,
    },
    CompileEquations {
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        artifacts: bool,
        #[arg(long, default_value_t = 4)]
        smoke_frames: u32,
    },
    Inspect {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::BeatAdaptCorpus { audit, output } => {
            let report = beat_adapt::frozen(&args.root, audit.as_deref(), &output)?;
            println!(
                "Frozen native pilot: {} generated / 100; {} matched predictions; {} failed.",
                report["generated"], report["matched_predictions"], report["failed"]
            );
        }
        Command::BeatRender {
            input,
            pcm,
            rate,
            channels,
            frames,
            output,
        } => {
            beat_adapt::render(
                &args.root,
                &input,
                pcm.as_deref(),
                rate,
                channels,
                frames,
                &output,
            )?;
        }
        Command::BeatAdaptReady { audit, output } => {
            let report = beat_adapt::ready(&args.root, &audit, &output)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::BeatExport { manifest, output } => beat::export(&args.root, &manifest, &output)?,
        Command::BeatInspect { input } => println!(
            "{}",
            serde_json::to_string_pretty(&beat::inspect(&args.root, &input)?)?
        ),
        Command::PreparePlaylist { input, rate } => comparison::playlist(&args.root, &input, rate)?,
        Command::PrepareComparison { input } => comparison::prepare(&args.root, &input)?,
        Command::PackagePoc => render::packages(&args.root)?,
        Command::RenderPoc {
            rank,
            frames,
            output,
        } => render::poc(&args.root, rank, frames, &output)?,
        Command::RenderFixed {
            rank,
            frames,
            output,
        } => render::fixed(&args.root, rank, frames, &output)?,
        Command::ValidateShader {
            spirv,
            wgsl,
            output,
        } => {
            let bytes = std::fs::read(&spirv)?;
            let shader = if wgsl {
                milk_compile::shader::from_wgsl(std::str::from_utf8(&bytes)?)?
            } else {
                milk_compile::shader::from_spirv(&bytes)?
            };
            let output = if output.is_absolute() {
                output
            } else {
                args.root.join(output)
            };
            write_report(&args.root, &output, &serde_json::to_string_pretty(&shader)?)?;
            println!(
                "Naga validated {} bindings; entry {}",
                shader.bindings.len(),
                shader.entry
            );
        }
        Command::CompileEquations {
            output,
            artifacts,
            smoke_frames,
        } => {
            let report = equations::report(&args.root, artifacts, smoke_frames)?;
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(path) = output {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    args.root.join(path)
                };
                write_report(&args.root, &absolute, &(json + "\n"))?;
                println!(
                    "Equation bundles: {}/100 compiled, {} smoke-passed. Shaders/render/4K30 remain pending.",
                    report["equations_compiled"], report["equation_smoke_passed"]
                );
            } else {
                println!("{json}");
            }
        }
        Command::Inspect { output } => {
            let c = corpus::verify(&args.root).context("frozen corpus verification failed")?;
            let mut outcomes = vec![];
            for entry in &c.presets {
                let bytes = corpus::verify_entry(&args.root, entry, "presets")?;
                let parsed = parser::parse(&bytes);
                let record = match parsed {
                    Ok(p) => {
                        serde_json::json!({"id":entry.id,"name":entry.path,"source_sha256":entry.sha256,"split":entry.split,"inspect":"parsed","conversion":"pending","render":"pending","visual_acceptance":"unverified","pcm_response":"unverified","native_4k30":"unmeasured","encoding":p.encoding,"static_fields":p.numeric.len(),"code_sections":p.sections.iter().map(|(k,v)|(k.clone(),v.len())).collect::<std::collections::BTreeMap<_,_>>(),"diagnostics":p.diagnostics})
                    }
                    Err(e) => {
                        serde_json::json!({"id":entry.id,"name":entry.path,"source_sha256":entry.sha256,"split":entry.split,"inspect":"failed","conversion":"pending","render":"pending","error":e.to_string()})
                    }
                };
                outcomes.push(record);
            }
            let report = serde_json::json!({"schema_version":1,"producer":"preset-cli-inspect-v1","manifest_sha256":corpus::sha256(&std::fs::read(args.root.join("corpus/manifest.json"))?),"population_sha256":c.population_manifest_sha256,"corpus_verified":true,"sample_denominator":100,"textures_verified":74,"parsed":outcomes.iter().filter(|r|r["inspect"]=="parsed").count(),"converted":0,"rendered":0,"qualified_4k30":0,"attribution":c.attribution,"presets":outcomes});
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(path) = output {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    args.root.join(path)
                };
                write_report(&args.root, &absolute, &(json + "\n"))?;
                println!(
                    "Corpus verified: 100 presets, 74 textures; parsed {}. Conversion/render/4K30 remain pending.",
                    report["parsed"]
                );
            } else {
                println!("{json}");
            }
        }
    }
    Ok(())
}

fn write_report(root: &std::path::Path, absolute: &std::path::Path, json: &str) -> Result<()> {
    write_bytes(root, absolute, json.as_bytes())
}
fn write_bytes(root: &std::path::Path, absolute: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let parent = absolute.parent().context("output path has no parent")?;
    let frozen = root.join("corpus").canonicalize()?;
    let ancestor = parent
        .ancestors()
        .find(|p| p.exists())
        .context("output path has no existing ancestor")?
        .canonicalize()?;
    anyhow::ensure!(
        !ancestor.starts_with(&frozen),
        "report cannot be written into frozen corpus"
    );
    match std::fs::symlink_metadata(absolute) {
        Ok(m) => anyhow::ensure!(
            !m.file_type().is_symlink(),
            "report destination must not be a symlink"
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    std::fs::create_dir_all(parent)?;
    let parent = parent.canonicalize()?;
    anyhow::ensure!(
        !parent.starts_with(&frozen),
        "report cannot be written into frozen corpus"
    );
    let name = absolute
        .file_name()
        .context("report path must name a file")?;
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temporary = parent.join(format!(
        ".preset-report-{}-{suffix}.tmp",
        std::process::id()
    ));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, parent.join(name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_rejection_does_not_create_directories_in_frozen_corpus() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        assert!(
            write_report(
                root.path(),
                &root.path().join("corpus/new/report.json"),
                "{}"
            )
            .is_err()
        );
        assert!(!root.path().join("corpus/new").exists());
    }
    #[cfg(unix)]
    #[test]
    fn report_writer_cannot_follow_a_symlink_into_frozen_corpus() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        std::fs::create_dir(root.path().join("reports")).unwrap();
        let frozen = root.path().join("corpus/manifest.json");
        std::fs::write(&frozen, "original").unwrap();
        let output = root.path().join("reports/out.json");
        std::os::unix::fs::symlink(&frozen, &output).unwrap();
        assert!(write_report(root.path(), &output, "changed").is_err());
        assert_eq!(std::fs::read_to_string(frozen).unwrap(), "original");
    }
}
