use crate::{convex_cache::refresh, mesh_math::V};

fn read(extra: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(extra[at..at + 4].try_into().unwrap())
}

fn put(extra: &mut [u8], at: usize, value: f32) {
    extra[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_v(extra: &mut [u8], at: usize, v: V) {
    put(extra, at, v.0);
    put(extra, at + 4, v.1);
    put(extra, at + 8, v.2);
}

fn chunk(verts: [V; 4]) -> Vec<u8> {
    let (polys, indices, n) = (1usize, 4usize, verts.len());
    let edges = n + polys - 2;
    let gap = edges * 2 + n * 3 + 4;
    let mut extra = vec![0u8; 24 + n * 12 + polys * 20 + indices + gap + 24];
    extra[..4].copy_from_slice(b"CLHL");
    for (at, value) in [(4, 9), (8, n), (12, edges), (16, polys), (20, indices)] {
        extra[at..at + 4].copy_from_slice(&(value as i32).to_le_bytes());
    }
    for (i, v) in verts.iter().enumerate() {
        put_v(&mut extra, 24 + i * 12, *v);
    }
    let poly_at = 24 + n * 12;
    put(&mut extra, poly_at + 4, 1.0);
    put(&mut extra, poly_at + 12, -1.0);
    extra[poly_at + 18] = 4;
    let index_at = poly_at + 20;
    extra[index_at..index_at + 4].copy_from_slice(&[0, 1, 2, 3]);
    let (min, max) = crate::convex_hull::bounds(verts);
    put_v(&mut extra, index_at + indices + gap, min);
    put_v(&mut extra, index_at + indices + gap + 12, max);
    extra
}

fn quad(x: f32, z: f32, sx: f32, sz: f32) -> [V; 4] {
    [
        V(x, 1.0, z),
        V(x + sx, 1.0, z),
        V(x + sx, 1.0, z + sz),
        V(x, 1.0, z + sz),
    ]
}

#[test]
fn shear_moves_the_cooked_hull_and_tilts_its_top_plane() {
    let mut extra = chunk(quad(0.0, 0.0, 2.0, 4.0));
    let updated = refresh(&mut extra, V(0.0, 1.0, 0.0), V(2.0, 1.0, 4.0), 0.05).unwrap();
    assert_eq!(updated, 1);
    assert_eq!(read(&extra, 28), 0.9);
    assert_eq!(read(&extra, 24 + 24 + 4), 1.1);
    assert_eq!(read(&extra, 24 + 48 + 8).to_bits(), 0xbd4c_8b67);
    assert_eq!(read(&extra, 122), 0.9);
    assert_eq!(read(&extra, 134), 1.1);
}

#[test]
fn shear_also_updates_the_mirrored_copy() {
    let mut extra = chunk(quad(0.0, 0.0, -2.0, -4.0));
    let updated = refresh(&mut extra, V(0.0, 1.0, 0.0), V(2.0, 1.0, 4.0), 0.05).unwrap();
    assert_eq!(updated, 1);
    assert_eq!(read(&extra, 28), 0.9);
    assert_eq!(read(&extra, 24 + 24 + 4), 1.1);
}

#[test]
fn a_chunk_outside_the_component_is_left_alone() {
    let mut extra = chunk(quad(10.0, 10.0, 2.0, 4.0));
    let before = extra.clone();
    let updated = refresh(&mut extra, V(0.0, 1.0, 0.0), V(2.0, 1.0, 4.0), 0.05).unwrap();
    assert_eq!(updated, 0);
    assert_eq!(extra, before);
}

#[test]
fn symmetric_hull_rejects_ambiguous_orientation() {
    let mut extra = chunk(quad(-1.0, -2.0, 2.0, 4.0));
    let error = refresh(&mut extra, V(-1.0, 1.0, -2.0), V(1.0, 1.0, 2.0), 0.05).unwrap_err();
    assert_eq!(error, "convex cache orientation is ambiguous");
}

#[test]
fn missing_bounds_record_fails_refresh() {
    let mut extra = chunk(quad(0.0, 0.0, 2.0, 4.0));
    let n = extra.len();
    extra[n - 24..].fill(0);
    let error = refresh(&mut extra, V(0.0, 1.0, 0.0), V(2.0, 1.0, 4.0), 0.05).unwrap_err();
    assert_eq!(error, "convex cache bounds record not found");
}
