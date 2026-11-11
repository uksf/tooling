use crate::{
    anim_edit_ops::add_hide,
    anim_ops::{add_translation, set_animation_source},
    memory_edit_ops::add_memory_point,
    odol::Model,
    op_json::{latin1_lossy, latin1_unit},
};
use serde_json::{Value, json};
use std::collections::HashMap;

const RHICC: &[u8] = include_bytes!("../tests/data/rhicc-memory.p3d");
const BEST_FIT_FIXTURE: &str = include_str!("../tests/data/latin1-best-fit.txt");

fn hide(bytes: &[u8], animation: &str, source: &str) -> Result<Vec<u8>, String> {
    let op =
        json!({"op":"add-hide","animation":animation,"bone":"OTOCVEZ","animationSource":source});
    add_hide(bytes, &op, "t")
}

fn last_class(bytes: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let m = Model::parse(bytes).unwrap();
    let c = m.animations.unwrap().classes.pop().unwrap();
    (c.name, c.source)
}

#[test]
fn latin1_unit_matches_dotnet_for_every_bmp_unit() {
    let fixture: HashMap<u16, u8> = BEST_FIT_FIXTURE
        .lines()
        .map(|line| {
            let (unit, byte) = line.split_once(' ').unwrap();
            (
                u16::from_str_radix(unit, 16).unwrap(),
                u8::from_str_radix(byte, 16).unwrap(),
            )
        })
        .collect();
    assert_eq!(fixture.len(), 295);
    for unit in 0..=u16::MAX {
        let expected =
            u8::try_from(unit).unwrap_or_else(|_| fixture.get(&unit).copied().unwrap_or(b'?'));
        assert_eq!(latin1_unit(unit), expected, "U+{unit:04X}");
    }
}

#[test]
fn latin1_lossy_spot_checks_match_dotnet() {
    assert_eq!(latin1_lossy("a\u{0100}b"), [97, 65, 98]);
    assert_eq!(latin1_lossy("a\u{212A}b"), [97, 63, 98]);
    assert_eq!(latin1_lossy("a\u{1F600}b"), [97, 63, 63, 98]);
    assert_eq!(latin1_lossy("\u{FF}"), [255]);
}

#[test]
fn add_hide_writes_the_best_fit_byte_for_u0100_in_the_animation() {
    let out = hide(RHICC, "hide_\u{0100}", "user").unwrap();
    assert_eq!(last_class(&out), (b"hide_A".to_vec(), b"user".to_vec()));
}

#[test]
fn add_hide_writes_the_best_fit_byte_for_u0100_in_the_animation_source() {
    let out = hide(RHICC, "hide_x", "source_\u{0100}").unwrap();
    assert_eq!(last_class(&out).1, b"source_A");
}

#[test]
fn add_hide_writes_a_question_mark_for_an_unmapped_bmp_char() {
    let out = hide(RHICC, "hide_\u{212A}", "user").unwrap();
    assert_eq!(last_class(&out).0, b"hide_?");
}

#[test]
fn add_hide_writes_two_question_marks_for_a_non_bmp_char() {
    let out = hide(RHICC, "hide_\u{1F600}", "user").unwrap();
    assert_eq!(last_class(&out).0, b"hide_??");
}

#[test]
fn add_hide_writes_u00ff_as_0xff() {
    let out = hide(RHICC, "hide_\u{FF}", "user").unwrap();
    assert_eq!(last_class(&out).0, b"hide_\xff");
}

#[test]
fn lookups_compare_strings_not_encoded_bytes() {
    let with_a = hide(RHICC, "hide_A", "user").unwrap();
    let with_question = hide(RHICC, "hide_?", "user").unwrap();
    for (bytes, name) in [
        (&with_a, "hide_\u{0100}"),
        (&with_question, "hide_\u{212A}"),
    ] {
        let op = json!({"animation":name,"to":"x"});
        assert!(
            set_animation_source(bytes, &op, "t")
                .unwrap_err()
                .contains("model has no animation")
        );
        let twice = hide(bytes, name, "user").unwrap();
        let classes = Model::parse(&twice).unwrap().animations.unwrap().classes;
        let n = classes.len();
        assert_eq!(classes[n - 1].name, classes[n - 2].name);
    }
    let op =
        json!({"op":"add-hide","animation":"h","bone":"OTOCVEZ\u{0100}","animationSource":"s"});
    assert!(
        add_hide(RHICC, &op, "t")
            .unwrap_err()
            .contains("skeleton has no bone")
    );
}

fn apply(op: fn(&[u8], &Value, &str) -> Result<Vec<u8>, String>, args: Value) -> Model {
    Model::parse(&op(RHICC, &args, "t").unwrap()).unwrap()
}

#[test]
fn set_animation_source_writes_the_best_fit_source() {
    let m = apply(
        set_animation_source,
        json!({"animation":"DAMAGEHIDEVEZ","to":"h\u{0100}\u{1F600}"}),
    );
    assert_eq!(m.animations.unwrap().classes[0].source, b"hA??");
}

#[test]
fn add_translation_writes_best_fit_name_and_source() {
    let bone = String::from_utf8(
        Model::parse(RHICC).unwrap().info.skeleton.bones[3]
            .name
            .clone(),
    )
    .unwrap();
    let m = apply(
        add_translation,
        json!({"animation":"s\u{0100}","bone":bone,"animationSource":"\u{212A}","minValue":0,"maxValue":1,
               "offset0":0,"offset1":1,"axis":[0,0,1]}),
    );
    let c = m.animations.unwrap().classes.pop().unwrap();
    assert_eq!((c.name, c.source), (b"sA".to_vec(), b"?".to_vec()));
}

#[test]
fn add_memory_point_writes_the_best_fit_name() {
    let m = apply(
        add_memory_point,
        json!({"name":"p\u{0100}","position":[0,0,0]}),
    );
    assert_eq!(m.lods[0].selections.last().unwrap().name, b"pA");
}
