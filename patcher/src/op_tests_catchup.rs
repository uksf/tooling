use crate::{
    add_points_ops::add_points,
    anim_edit_ops::{add_hide, move_rotation_axis, set_rotation_range},
    odol::Model,
    point_ops::{add_selection_points, isolate_memory_selection},
};
use serde_json::{Value, json};

type Op = fn(&[u8], &Value, &str) -> Result<Vec<u8>, String>;

const RHICC: &[u8] = include_bytes!("../tests/data/rhicc-memory.p3d");

fn apply(op: Op, args: Value) -> Model {
    Model::parse(&op(RHICC, &args, "t").unwrap()).unwrap()
}

fn named<'a>(m: &'a Model, lod: usize, name: &str) -> &'a crate::odol::Selection {
    m.lods[lod]
        .selections
        .iter()
        .find(|s| s.name == name.as_bytes())
        .unwrap()
}

#[test]
fn add_hide_binds_a_hide_class_to_the_bone_in_every_lod() {
    let op = json!({"op":"add-hide","animation":"toe_hide","bone":"OTOCVEZ","animationSource":"toe_src"});
    let m = apply(add_hide, op.clone());
    let an = m.animations.as_ref().unwrap();
    let c = an.classes.last().unwrap();
    assert_eq!(
        (
            c.kind,
            c.name.as_slice(),
            c.source.as_slice(),
            c.hide,
            c.unused55
        ),
        (9, &b"toe_hide"[..], &b"toe_src"[..], 0.5, -1.0)
    );
    assert_eq!(
        (c.min_phase, c.max_phase, c.min_value, c.max_value),
        (0.0, 1.0, 0.0, 1.0)
    );
    assert_eq!(an.bones_to_anims[0][1].last(), Some(&48));
    assert_eq!((an.anims_to_bones[0][48], an.axes[0][48]), (1, None));
    let twice = add_hide(&m.write().unwrap(), &op, "t").unwrap_err();
    assert_eq!(twice, "t: model already has an animation 'toe_hide'");
    let bad = json!({"animation":"x","bone":"missing","animationSource":"x"});
    assert_eq!(
        add_hide(RHICC, &bad, "t").unwrap_err(),
        "t: skeleton has no bone 'missing'"
    );
}

#[test]
fn set_rotation_range_changes_only_the_named_rotation() {
    let m = apply(
        set_rotation_range,
        json!({"animation":"codriver1turret","angle0":-1.3,"angle1":1.75}),
    );
    let c = &m.animations.as_ref().unwrap().classes;
    assert_eq!((c[2].angle0, c[2].angle1), (-1.3, 1.75));
    let before = Model::parse(RHICC).unwrap();
    let old = &before.animations.as_ref().unwrap().classes[3];
    assert_eq!((c[3].angle0, c[3].angle1), (old.angle0, old.angle1));
    for name in ["missing", "codriver1gunshake"] {
        let op = json!({"animation":name,"angle0":0,"angle1":1});
        assert_eq!(
            set_rotation_range(RHICC, &op, "t").unwrap_err(),
            format!("t: expected one rotation animation '{name}'")
        );
    }
}

#[test]
fn move_rotation_axis_moves_every_stored_origin_and_both_memory_points() {
    let op =
        json!({"animation":"drivingwheel","selection":"drivewheel_axis","offset":[0,0.08,-0.176]});
    let m = apply(move_rotation_axis, op);
    let before = Model::parse(RHICC).unwrap();
    let (old, new) = (
        before.animations.as_ref().unwrap(),
        m.animations.as_ref().unwrap(),
    );
    for (a, b) in old.axes.iter().zip(&new.axes) {
        let (a, b) = (a[26].unwrap(), b[26].unwrap());
        assert_eq!(b[0], [a[0][0], a[0][1] + 0.08, a[0][2] - 0.176]);
        assert_eq!(b[1], a[1]);
    }
    assert_eq!(
        new.axes[0][26].unwrap()[0].map(f32::to_bits),
        [0x3ef2_f437, 0xbc42_74b0, 0xbe0e_1560]
    );
    for &v in &named(&before, 0, "drivewheel_axis").vertices {
        let (a, b) = (
            before.lods[0].vertices[v as usize],
            m.lods[0].vertices[v as usize],
        );
        assert_eq!(b, [a[0], a[1] + 0.08, a[2] - 0.176]);
    }
    let missing = json!({"animation":"missing","selection":"drivewheel_axis","offset":[0,0,0]});
    assert_eq!(
        move_rotation_axis(RHICC, &missing, "t").unwrap_err(),
        "t: expected one rotation animation 'missing'"
    );
}

#[test]
fn isolate_memory_selection_moves_a_shared_point_onto_a_new_bound_vertex() {
    let before = Model::parse(RHICC).unwrap();
    let mem = &before.lods[0];
    let shared = mem
        .selections
        .iter()
        .find(|s| {
            s.vertices.len() == 1
                && mem
                    .selections
                    .iter()
                    .any(|o| o.name != s.name && o.vertices.contains(&s.vertices[0]))
        })
        .unwrap();
    let name = String::from_utf8(shared.name.clone()).unwrap();
    let old = shared.vertices[0] as usize;
    let op = json!({"name":name,"position":[1.25,2.5,3.75],"bone":"otocvez"});
    let m = apply(isolate_memory_selection, op);
    let lod = &m.lods[0];
    assert_eq!(named(&m, 0, &name).vertices, vec![152]);
    assert_eq!(lod.vertices[152], [1.25, 2.5, 3.75]);
    assert_eq!(lod.vertices[old], mem.vertices[old]);
    let sub = lod.sub_to_skeleton.iter().position(|&b| b == 1).unwrap();
    assert_eq!(
        (lod.bones[152].count, lod.bones[152].data[..2].to_vec()),
        (1, vec![sub as u8, 255])
    );
}

#[test]
fn add_points_creates_the_hit_points_lod_from_memory() {
    let op = json!({"lod":"hitpoints","name":"HitTest","points":2,"positions":[[1,2,3],[4,5,6]]});
    let m = apply(add_points, op);
    assert_eq!(
        (m.lods.len(), m.lods[1].resolution, m.info.lod_index[11]),
        (2, 5e15, 1)
    );
    let hit = named(&m, 1, "HitTest");
    assert_eq!(hit.vertices, vec![152, 153]);
    assert_eq!(m.lods[1].selections.len(), 1);
    assert_eq!(m.lods[1].vertices[153], [4.0, 5.0, 6.0]);
    let wrong = json!({"lod":"hitpoints","name":"HitTest","points":3,"positions":[[1,2,3]]});
    assert_eq!(
        add_points(RHICC, &wrong, "t").unwrap_err(),
        "t: 'HitTest' has 1 points, the patch expects 3"
    );
}

#[test]
fn add_selection_points_adds_loose_points_and_refuses_short_ones() {
    let op = json!({"lod":"memory","name":"fuel tanks","points":[[1.5,2,-0.5],[-1.5,2,-0.5]]});
    let m = apply(add_selection_points, op);
    assert_eq!(named(&m, 0, "fuel tanks").vertices, vec![152, 153]);
    assert_eq!(m.lods[0].vertices[152], [1.5, 2.0, -0.5]);
    let short = json!({"lod":"memory","name":"fuel tanks","points":[[1,2]]});
    assert_eq!(
        add_selection_points(RHICC, &short, "t").unwrap_err(),
        "t: add-selection-points 'fuel tanks' needs [x, y, z] for every point"
    );
}

#[test]
fn add_points_serializes_overflow_as_infinity() {
    let op: Value = serde_json::from_str(
        r#"{"lod":"hitpoints","name":"HitTest","points":1,"positions":[[1e39,-1e39,0]]}"#,
    )
    .unwrap();
    let m = apply(add_points, op);
    assert_eq!(
        m.lods[1].vertices[152].map(f32::to_bits),
        [0x7f80_0000, 0xff80_0000, 0]
    );
}

#[test]
fn add_selection_points_serializes_overflow_as_infinity() {
    let op: Value =
        serde_json::from_str(r#"{"lod":"memory","name":"overflow","points":[[1e39,-1e39,0]]}"#)
            .unwrap();
    let m = apply(add_selection_points, op);
    assert_eq!(
        m.lods[0].vertices[152].map(f32::to_bits),
        [0x7f80_0000, 0xff80_0000, 0]
    );
}
