use milk_import::parser::parse;

#[test]
fn sparse_sections_are_numerically_ordered_and_source_mapped() {
    let p = parse(b"[preset00]\r\nfDecay=.97\r\nper_frame_10=b=2;\r\nper_frame_2=a=1;\r\nwarp_1=`shader_body\r\nwarp_3=`{ ret=float3(1,0,0); }\r\n").unwrap();
    assert_eq!(p.numeric["fDecay"], 0.97);
    assert_eq!(p.sections["per_frame"][0].number, 2);
    assert_eq!(p.sections["per_frame"][0].source_line, 4);
    assert_eq!(p.sections["warp"][0].text, "shader_body");
    assert_eq!(p.sections["warp"][1].number, 3);
}

#[test]
fn custom_wave_shape_scopes_and_duplicate_precedence_survive() {
    let p=parse(b"[preset00]\nzoom=1\nzoom=2\nwave_0_init1=t1=3;\nwave_0_per_point1=x=sample;\nshape_2_per_frame1=x=q1;\n").unwrap();
    assert_eq!(p.numeric["zoom"], 2.0);
    assert_eq!(p.sections["wave_0_init"].len(), 1);
    assert_eq!(p.sections["wave_0_per_point"][0].text, "x=sample;");
    assert_eq!(p.sections["shape_2_per_frame"].len(), 1);
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.message.contains("duplicate"))
    );
}

#[test]
fn utf8_and_legacy_encoding_are_explicit_and_invalid_sources_rejected() {
    let p = parse("[preset00]\nname=étoile\n".as_bytes()).unwrap();
    assert_eq!(p.text["name"], "étoile");
    assert_eq!(p.encoding, "utf-8");
    let p = parse(b"[preset00]\nname=\xe9toile\n").unwrap();
    assert_eq!(p.text["name"], "étoile");
    assert_eq!(p.encoding, "windows-1252");
    assert!(parse(b"zoom=1").is_err());
    assert!(parse(&vec![b'a'; 512 * 1024 + 1]).is_err());
}
