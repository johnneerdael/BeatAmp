use effect_runtime::state::{FrameInput, PresetState};
use milk_compile::bundle::compile_equations;
use milk_import::parser::parse;

#[test]
fn frame_q_resets_user_variables_persist_and_scopes_stay_separate() {
    let p=parse(b"[preset00]\nzoom=1\nfDecay=.98\nper_frame_init_1=q1=10;counter=0;\nper_frame_1=counter+=1;q1+=counter;zoom+=q1*.01;\nper_pixel_1=q1+=1;zoom+=x;\nwavecode_0_enabled=1\nwavecode_0_samples=8\nwave_0_init1=t1=5;wcount=0;\nwave_0_per_frame1=wcount+=1;t1+=wcount;\nwave_0_per_point1=t1+=1;x=q1+t1;\n").unwrap();
    let mut s = PresetState::new(compile_equations(&p).unwrap(), 12345).unwrap();
    s.frame(FrameInput::default()).unwrap();
    assert_eq!(s.frame_value("q1"), 11.0);
    assert!((s.frame_value("zoom") - 1.11).abs() < 1e-12);
    s.pixel(0.25, 0.5).unwrap();
    assert_eq!(s.pixel_value("q1"), 12.0);
    assert_eq!(s.frame_value("q1"), 11.0);
    s.begin_wave(0).unwrap();
    s.wave_point(0, 0.0, 0.0, 0.0).unwrap();
    assert_eq!(s.wave_point_value(0, "x"), 18.0);
    s.wave_point(0, 0.5, 0.0, 0.0).unwrap();
    assert_eq!(s.wave_point_value(0, "x"), 19.0);
    s.frame(FrameInput {
        frame: 1,
        time: 1.0 / 30.0,
        ..FrameInput::default()
    })
    .unwrap();
    assert_eq!(s.frame_value("counter"), 2.0);
    assert_eq!(s.frame_value("q1"), 12.0);
    assert!((s.frame_value("zoom") - 1.12).abs() < 1e-12);
    s.begin_wave(0).unwrap();
    s.wave_point(0, 0.0, 0.0, 0.0).unwrap();
    assert_eq!(s.wave_point_value(0, "x"), 20.0);
}

#[test]
fn global_memory_is_shared_within_a_preset_and_never_across_presets() {
    let p=parse(b"[preset00]\nper_frame_init_1=gmegabuf(2)=7;\nper_frame_1=a=gmegabuf(2);\nshape_0_per_frame1=x=gmegabuf(2);gmegabuf(2)+=1;\n").unwrap();
    let bundle = compile_equations(&p).unwrap();
    let mut one = PresetState::new(bundle.clone(), 1).unwrap();
    let mut two = PresetState::new(bundle, 1).unwrap();
    one.frame(FrameInput::default()).unwrap();
    one.shape(0, 0).unwrap();
    one.frame(FrameInput::default()).unwrap();
    two.frame(FrameInput::default()).unwrap();
    assert_eq!(one.frame_value("a"), 8.0);
    assert_eq!(two.frame_value("a"), 7.0);
}

#[test]
fn authored_large_memory_initialization_uses_the_initialization_budget() {
    let source = include_bytes!(
        "../../../corpus/presets/Fumbling_Foo & Flexi, Martin, Orb - Star Forge v12b.milk"
    );
    let p = parse(source).unwrap();
    let mut s = PresetState::new(compile_equations(&p).unwrap(), 12345).unwrap();
    assert!(s.memory_bytes() > 1_600_000 && s.memory_bytes() < 4_000_000);
    s.frame(FrameInput::default()).unwrap();
    assert!(s.instruction_count() < 4_000_000);
}
