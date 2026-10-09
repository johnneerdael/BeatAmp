use effect_runtime::vm::{Memory, Random, Vm};
use milk_compile::equations::{compile, compile_lines};
use milk_import::parser::SourceLine;

fn run(source: &str) -> (f64, Vm) {
    let p = compile(source).unwrap();
    let mut vm = Vm::new(&p).unwrap();
    let value = vm
        .execute(
            &p,
            &mut Memory::default(),
            &mut Random::new(12345),
            1_000_000,
        )
        .unwrap();
    (value, vm)
}

#[test]
fn source_order_precision_and_precedence_are_executed() {
    let (v, vm) = run("a=2+3*4;b=a^2;huge=16777217;exact=huge-16777216;b+exact;");
    assert_eq!(v, 197.0);
    assert_eq!(vm.get("a"), 14.0);
    assert_eq!(vm.get("exact"), 1.0);
    let (v, _) = run("-2^2 + 2^3^2");
    assert_eq!(v, 508.0);
}

#[test]
fn lazy_if_and_boolean_operators_preserve_assignment_order() {
    let (v, vm) = run("a=0;if(0,a=99,a=4);b=0 && (a=7);c=1 || (a=9);bor(a=5,a=6);if(1e-6,99,a);");
    assert_eq!(v, 6.0);
    assert_eq!(vm.get("a"), 6.0);
    assert_eq!(vm.get("b"), 0.0);
    assert_eq!(vm.get("c"), 1.0);
}

#[test]
fn loops_and_memory_lvalues_are_ordered_and_shared_only_explicitly() {
    let (v, vm) = run(
        "i=0;loop(10,gmegabuf(i)=sqr(i);i+=1;);x=gmegabuf(3);megabuf(0)=7;megabuf(0)+=2;gmegabuf(3)+megabuf(0);",
    );
    assert_eq!(v, 18.0);
    assert_eq!(vm.get("x"), 9.0);
    assert_eq!(vm.get("i"), 10.0);
    let (v, _) = run("x=1;loop(0,x=99);x");
    assert_eq!(v, 1.0);
}

#[test]
fn source_numeric_functions_follow_characterized_rules() {
    let (v, vm) = run("a=int(-1.2);b=sqrt(-9);c=equal(1,1.000001);d=sign(0);e=(-5)%3;a+b+c+d+e");
    assert_eq!(vm.get("a"), -2.0);
    assert_eq!(vm.get("b"), 3.0);
    assert_eq!(v, 4.0);
}

#[test]
fn seed_repeat_and_lazy_rand_have_identical_streams() {
    let source = "a=rand(100);b=if(0,rand(100),0);c=rand(100);a+c";
    let (a, va) = run(source);
    let (b, vb) = run(source);
    assert!(a > 0.0);
    assert_eq!(a, b);
    assert_eq!(va.get("c"), vb.get("c"));
    let (_, vc) = run("a=rand(100);c=rand(100)");
    assert_eq!(va.get("c"), vc.get("c"));
}

#[test]
fn resource_limits_and_unsupported_constructs_produce_diagnostics() {
    assert!(compile("unknown(1)").is_err());
    assert!(compile("a=.").is_err());
    assert!(compile("a=1e999").is_err());
    assert!(compile(&format!("{}1{}", "(".repeat(200), ")".repeat(200))).is_err());
    let p = compile("loop(1000000,x+=1)").unwrap();
    let mut vm = Vm::new(&p).unwrap();
    assert!(
        vm.execute(&p, &mut Memory::default(), &mut Random::new(1), 100)
            .is_err()
    );
    let p = compile("gmegabuf(1048576)=1").unwrap();
    let mut vm = Vm::new(&p).unwrap();
    assert!(
        vm.execute(&p, &mut Memory::default(), &mut Random::new(1), 100)
            .is_err()
    );
}

#[test]
fn diagnostics_reference_original_numbered_source_lines() {
    let lines = vec![
        SourceLine {
            number: 1,
            source_line: 300,
            text: "a=1; // ignored\n".into(),
        },
        SourceLine {
            number: 2,
            source_line: 405,
            text: "b=unknown(2);".into(),
        },
    ];
    let error = compile_lines(&lines).unwrap_err().to_string();
    assert!(error.contains("405"), "{error}");
}
