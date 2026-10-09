use effect_model::beat::{Capabilities, Manifest};
use serde_json::{Value, json};

fn manifest_json() -> Value {
    json!({
        "format":"beat", "version":1, "name":"Portal", "author":"Test artist",
        "required_features":[], "quality":"native", "output":"final",
        "assets":{"field":{"path":"programs/field.wgsl","sha256":"a".repeat(64),"kind":"wgsl"}},
        "parameters":{"portal.radius":{"initial":[0.74]},"palette.warmth":{"initial":[0.8]}},
        "resources":{
            "history":{"kind":"texture","format":"rgba8unorm","extent":{"mode":"native"},"history":true,"role":"primary_feedback","mips":1,"color_space":"linear","alpha":"premultiplied"},
            "final":{"kind":"texture","format":"rgba8unorm","extent":{"mode":"native"},"history":false,"role":"output","mips":1,"color_space":"linear","alpha":"opaque"}
        },
        "passes":[
            {"id":"display","kind":"render","program":"field","reads":[{"resource":"history","previous":false,"kind":"color_texture"}],"writes":["final"]},
            {"id":"inject","kind":"render","program":"field","reads":[{"resource":"history","previous":true,"kind":"color_texture"}],"writes":["history"]}
        ]
    })
}

#[test]
fn executable_bindings_preserve_sampler_axes_and_cannot_bypass_graph_dependencies() {
    // Missing executable metadata or binding undeclared resources breaks GPU planning.
    let mut value = manifest_json();
    value["passes"][1]["parameters"] = json!(["portal.radius"]);
    value["passes"][1]["bindings"] = json!([
        {"binding":0,"source":{"kind":"parameters"}},
        {"binding":1,"source":{"kind":"resource","resource":"history","previous":true}},
        {"binding":2,"source":{"kind":"sampler","u":"repeat","v":"clamp-to-edge","filter":"linear"}}
    ]);
    value["passes"][1]["draw"] = json!({"vertices":6,"instances":12,"topology":"triangle-list"});
    let manifest = parse(value.clone());
    assert!(
        manifest
            .validate(&Capabilities::baseline([3840, 2160]))
            .is_ok()
    );
    let encoded = serde_json::to_value(manifest).unwrap();
    assert_eq!(
        encoded["passes"][1]["bindings"][2]["source"]["v"],
        "clamp-to-edge"
    );
    assert_eq!(encoded["passes"][1]["draw"]["instances"], 12);
    value["passes"][1]["bindings"][1]["source"]["previous"] = json!(false);
    assert!(
        parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .is_err()
    );
}

#[test]
fn executable_draw_and_dispatch_work_are_bounded_before_gpu_submission() {
    // Unbounded instance/dispatch work can pass allocation checks but stall rendering.
    let mut value = manifest_json();
    value["passes"][0]["draw"] =
        json!({"vertices":u32::MAX,"instances":u32::MAX,"topology":"triangle-list"});
    assert!(
        parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .is_err()
    );
    let mut value = manifest_json();
    value["resources"]["points"] =
        json!({"kind":"buffer","bytes":16,"initialized":true,"history":false});
    value["passes"].as_array_mut().unwrap().push(json!({"id":"step","kind":"compute","program":"field","reads":[],"writes":["points"],"dispatch":[65535,65535,65535]}));
    assert!(
        parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .is_err()
    );
}

fn parse(value: Value) -> Manifest {
    serde_json::from_value(value).unwrap()
}

#[test]
fn orders_native_feedback_passes_and_counts_both_history_textures() {
    // A planner that treats previous-frame feedback as a same-frame cycle fails.
    let manifest = parse(manifest_json());
    let plan = manifest
        .validate(&Capabilities::baseline([3840, 2160]))
        .unwrap();
    assert_eq!(plan.pass_order, [1, 0]);
    assert_eq!(plan.resource_bytes, 99_532_800);
    assert_eq!(plan.dimensions["history"], [3840, 2160]);
}

#[test]
fn rejects_cycles_multiple_writers_and_missing_source_resources() {
    for (change, reason) in [
        (0, "cycle"),
        (1, "multiple writers"),
        (2, "unknown resource"),
        (3, "history"),
        (4, "binding type"),
        (5, "program"),
    ] {
        let mut value = manifest_json();
        match change {
            0 => value["passes"][1]["reads"][0]["previous"] = json!(false),
            1 => value["passes"][0]["writes"] = json!(["history"]),
            2 => value["passes"][0]["reads"][0]["resource"] = json!("missing"),
            3 => value["resources"]["history"]["history"] = json!(false),
            4 => value["passes"][0]["reads"][0]["kind"] = json!("depth_texture"),
            5 => value["passes"][0]["program"] = json!("missing"),
            _ => unreachable!(),
        }
        let error = parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .unwrap_err();
        assert!(error.to_string().contains(reason), "{change}: {error:#}");
    }
}

#[test]
fn rejects_hidden_reduced_feedback_and_oversized_allocations() {
    let mut value = manifest_json();
    value["resources"]["history"]["extent"] = json!({"mode":"fixed","width":1280,"height":720});
    assert!(
        parse(value.clone())
            .validate(&Capabilities::baseline([3840, 2160]))
            .unwrap_err()
            .to_string()
            .contains("native")
    );
    value["quality"] = json!("reduced");
    assert!(
        parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .is_ok()
    );
    let mut caps = Capabilities::baseline([3840, 2160]);
    caps.max_resource_bytes = 99_532_799;
    assert!(
        parse(manifest_json())
            .validate(&caps)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
}

#[test]
fn accounts_for_4k_master_mips_without_the_old_32_mib_limit() {
    let mut value = manifest_json();
    value["resources"]["image"] = json!({"kind":"texture","format":"rgba8unorm-srgb","extent":{"mode":"fixed","width":4096,"height":4096},"history":false,"role":"asset","mips":13,"color_space":"srgb","alpha":"straight","asset":"image"});
    value["assets"]["image"] =
        json!({"path":"textures/color.png","sha256":"b".repeat(64),"kind":"image"});
    let plan = parse(value)
        .validate(&Capabilities::baseline([3840, 2160]))
        .unwrap();
    assert_eq!(plan.resource_bytes, 189_011_284);
}

#[test]
fn cooked_texture_mips_can_use_declared_binary_payloads() {
    // A PNG-only asset-kind check prevents precise signed-flow/mip cooking.
    let mut value = manifest_json();
    value["resources"]["flow"] = json!({"kind":"texture","format":"rg16float","extent":{"mode":"fixed","width":4096,"height":4096},"history":false,"role":"asset","mips":13,"color_space":"data","alpha":"opaque","asset":"flow"});
    value["assets"]["flow"] =
        json!({"path":"textures/flow.raw","sha256":"f".repeat(64),"kind":"binary"});
    let manifest = parse(value);
    assert!(
        manifest
            .validate(&Capabilities::baseline([3840, 2160]))
            .is_ok()
    );
}

#[test]
fn accepts_color_depth_and_compute_in_the_same_manifest() {
    let mut value = manifest_json();
    value["resources"]["depth"] = json!({"kind":"texture","format":"depth32float","extent":{"mode":"native"},"history":false,"role":"intermediate","mips":1,"color_space":"data","alpha":"opaque"});
    value["resources"]["instances"] =
        json!({"kind":"buffer","bytes":192,"history":false,"initialized":true});
    value["passes"].as_array_mut().unwrap().push(json!({"id":"simulate","kind":"compute","program":"field","reads":[],"writes":["instances"]}));
    value["passes"][1]["reads"]
        .as_array_mut()
        .unwrap()
        .push(json!({"resource":"instances","previous":false,"kind":"buffer"}));
    value["passes"][1]["writes"]
        .as_array_mut()
        .unwrap()
        .push(json!("depth"));
    let manifest = parse(value);
    let plan = manifest
        .validate(&Capabilities::baseline([3840, 2160]))
        .unwrap();
    assert_eq!(plan.pass_order, [2, 1, 0]);
    assert_eq!(plan.resource_bytes, 132_710_592);
    assert!(serde_json::from_slice::<Manifest>(&serde_json::to_vec(&manifest).unwrap()).is_ok());
}

#[test]
fn requires_enabled_wgpu_features_for_normalized_16_bit_maps() {
    let mut value = manifest_json();
    value["resources"]["height"] = json!({"kind":"texture","format":"r16unorm","extent":{"mode":"fixed","width":4096,"height":4096},"history":false,"role":"asset","mips":1,"color_space":"data","alpha":"opaque","asset":"height"});
    value["assets"]["height"] =
        json!({"path":"textures/height.png","sha256":"c".repeat(64),"kind":"image"});
    let manifest = parse(value);
    let mut caps = Capabilities::baseline([3840, 2160]);
    assert!(
        manifest
            .validate(&caps)
            .unwrap_err()
            .to_string()
            .contains("feature")
    );
    caps.features |= wgpu_types::Features::TEXTURE_FORMAT_16BIT_NORM;
    assert_eq!(
        manifest.validate(&caps).unwrap().resource_bytes,
        133_087_232
    );
}

#[test]
fn shared_texture_inputs_cannot_be_used_as_mutable_attachments() {
    let mut value = manifest_json();
    value["resources"]["history"]["role"] = json!("asset");
    value["resources"]["history"]["asset"] = json!("image");
    value["assets"]["image"] =
        json!({"path":"textures/image.png","sha256":"d".repeat(64),"kind":"image"});
    let error = parse(value)
        .validate(&Capabilities::baseline([3840, 2160]))
        .unwrap_err();
    assert!(error.to_string().contains("immutable"));
}

#[test]
fn parameters_are_named_values_without_a_legacy_register_count() {
    let mut value = manifest_json();
    for i in 0..128 {
        value["parameters"][format!("material.channel{i}")] = json!({"initial":[0.0,0.25,0.5,1.0]});
    }
    let mut manifest = parse(value);
    assert_eq!(
        manifest
            .validate(&Capabilities::baseline([3840, 2160]))
            .unwrap()
            .parameter_bytes,
        2056
    );
    manifest
        .parameters
        .get_mut("portal.radius")
        .unwrap()
        .initial[0] = f32::NAN;
    assert!(
        manifest
            .validate(&Capabilities::baseline([3840, 2160]))
            .unwrap_err()
            .to_string()
            .contains("nonfinite")
    );
}

#[test]
fn color_interpretation_and_asset_paths_are_validated_before_upload() {
    for (change, reason) in [(0, "color-space"), (1, "asset path"), (2, "asset hash")] {
        let mut value = manifest_json();
        match change {
            0 => value["resources"]["history"]["color_space"] = json!("srgb"),
            1 => value["assets"]["field"]["path"] = json!("../outside.wgsl"),
            2 => value["assets"]["field"]["sha256"] = json!("bad"),
            _ => unreachable!(),
        }
        assert!(
            parse(value)
                .validate(&Capabilities::baseline([3840, 2160]))
                .unwrap_err()
                .to_string()
                .contains(reason)
        );
    }
}

#[test]
fn compressed_master_extent_respects_wgpu_block_alignment() {
    let mut value = manifest_json();
    value["resources"]["image"] = json!({"kind":"texture","format":"bc1-rgba-unorm","extent":{"mode":"fixed","width":5,"height":4},"history":false,"role":"asset","mips":1,"color_space":"linear","alpha":"opaque","asset":"image"});
    value["assets"]["image"] =
        json!({"path":"textures/image.bin","sha256":"f".repeat(64),"kind":"image"});
    let mut caps = Capabilities::baseline([3840, 2160]);
    caps.features |= wgpu_types::Features::TEXTURE_COMPRESSION_BC;
    assert!(
        parse(value)
            .validate(&caps)
            .unwrap_err()
            .to_string()
            .contains("block")
    );
}

#[test]
fn attachment_sets_respect_extent_depth_count_and_device_color_budget() {
    for (case, expected) in [
        (0, "attachment extent"),
        (1, "depth attachment"),
        (2, "attachment count"),
        (3, "attachment byte"),
    ] {
        let mut value = manifest_json();
        let count = match case {
            0 => 1,
            1 => 2,
            2 => 9,
            _ => 5,
        };
        let mut writes = if case == 2 || case == 3 {
            vec![]
        } else {
            vec![json!("history")]
        };
        for i in 0..count {
            let name = format!("target{i}");
            let format = if case == 1 {
                "depth32float"
            } else {
                "rgba8unorm"
            };
            let space = if case == 1 { "data" } else { "linear" };
            value["resources"][&name] = json!({"kind":"texture","format":format,"extent":if case==0{json!({"mode":"fixed","width":1280,"height":720})}else{json!({"mode":"native"})},"history":false,"role":"intermediate","mips":1,"color_space":space,"alpha":"opaque"});
            writes.push(json!(name));
        }
        value["passes"][1]["writes"] = json!(writes);
        let error = parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{case}: {error:#}");
    }
}

#[test]
fn unused_allocatable_texture_cannot_reach_wgpu_with_empty_usage() {
    let mut value = manifest_json();
    value["resources"]["orphan"] = json!({"kind":"texture","format":"rgba8unorm","extent":{"mode":"fixed","width":1,"height":1},"history":false,"role":"intermediate","mips":1,"color_space":"linear","alpha":"opaque"});
    assert!(
        parse(value)
            .validate(&Capabilities::baseline([3840, 2160]))
            .unwrap_err()
            .to_string()
            .contains("unused texture")
    );
}
