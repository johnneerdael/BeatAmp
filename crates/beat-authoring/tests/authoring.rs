#![cfg(feature = "offline")]
use beat_authoring::{SourceHints, SynthesisRequest, synthesize};
use effect_model::beat::{Capabilities, Extent, Resource, TextureRole};
use std::io::Cursor;

fn source(index: usize) -> SourceHints {
    SourceHints::from_bytes(format!("[preset00]\nfDecay=0.96\nfWaveScale={}\nwave_r=0.2\nwave_g=0.7\nwave_b=1\nper_frame_1=rot=sin(time);\n", index as f32 * 0.1).as_bytes()).unwrap()
}

#[test]
fn deterministic_standalone_native_graph_and_valid_programs() {
    let hints = source(1);
    let caps = Capabilities::baseline([3840, 2160]);
    let request = SynthesisRequest {
        name: "Native test",
        source: &hints,
        prediction: None,
        capabilities: &caps,
    };
    let first = synthesize(&request).unwrap();
    let second = synthesize(&request).unwrap();
    let mut a = Cursor::new(Vec::new());
    let mut b = Cursor::new(Vec::new());
    first.package.write(&mut a).unwrap();
    second.package.write(&mut b).unwrap();
    assert_eq!(a.get_ref(), b.get_ref());
    let reloaded = effect_package::Package::read(a, &Default::default()).unwrap();
    let plan = reloaded.manifest.validate(&caps).unwrap();
    assert_eq!(plan.dimensions[&reloaded.manifest.output], [3840, 2160]);
    for resource in reloaded.manifest.resources.values() {
        if let Resource::Texture { role, extent, .. } = resource
            && matches!(role, TextureRole::Output | TextureRole::PrimaryFeedback)
        {
            assert!(matches!(extent, Extent::Native));
        }
    }
    for (id, asset) in &reloaded.manifest.assets {
        if asset.kind == effect_model::beat::AssetKind::Wgsl {
            let module =
                naga::front::wgsl::parse_str(std::str::from_utf8(&reloaded.assets[id]).unwrap())
                    .unwrap();
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap();
        }
    }
    assert!(
        !reloaded
            .assets
            .values()
            .any(|v| v.windows(10).any(|s| s == b"[preset00]"))
    );
    assert_eq!(first.metadata.prediction_status, "pending");
}

#[test]
fn source_identities_choose_different_component_compositions() {
    let caps = Capabilities::baseline([3840, 2160]);
    let mut compositions = std::collections::BTreeSet::new();
    let mut graph_shapes = std::collections::BTreeSet::new();
    for index in 0..32 {
        let hints = source(index);
        let result = synthesize(&SynthesisRequest {
            name: "Variation",
            source: &hints,
            prediction: None,
            capabilities: &caps,
        })
        .unwrap();
        compositions.insert(result.metadata.components);
        graph_shapes.insert(result.package.manifest.passes.len());
    }
    assert!(
        compositions.len() >= 12,
        "composition should vary beyond recoloring"
    );
    assert!(graph_shapes.len() >= 2, "feedback is optional graph work");
}
