use crate::{convex_hull::bounds, mesh_math::V};

const SIGNATURE: &[u8] = b"CLHL";

struct Chunk {
    vert_at: usize,
    verts: usize,
    edges: usize,
    poly_at: usize,
    polys: usize,
    index_at: usize,
    indices: usize,
    min: V,
    max: V,
}

fn i32_at(extra: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([extra[at], extra[at + 1], extra[at + 2], extra[at + 3]])
}

fn read(extra: &[u8], at: usize) -> f32 {
    f32::from_bits(i32_at(extra, at) as u32)
}

fn write(extra: &mut [u8], at: usize, value: f32) {
    extra[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_v(extra: &[u8], at: usize) -> V {
    V(read(extra, at), read(extra, at + 4), read(extra, at + 8))
}

fn write_v(extra: &mut [u8], at: usize, v: V) {
    write(extra, at, v.0);
    write(extra, at + 4, v.1);
    write(extra, at + 8, v.2);
}

fn chunk_bounds(extra: &[u8], at: usize, count: usize) -> (V, V) {
    bounds((0..count).map(|i| read_v(extra, at + i * 12)))
}

fn valid(extra: &[u8], at: usize) -> Option<Chunk> {
    let version = i32_at(extra, at + 4);
    let verts = i32_at(extra, at + 8);
    let edges = i32_at(extra, at + 12);
    let polys = i32_at(extra, at + 16);
    let indices = i32_at(extra, at + 20);
    if !(1..=32).contains(&version)
        || !(4..=64).contains(&verts)
        || !(1..=64).contains(&polys)
        || edges != verts + polys - 2
        || indices < polys
        || indices > polys * 16
    {
        return None;
    }
    let (verts, edges, polys, indices) = (
        verts as usize,
        edges as usize,
        polys as usize,
        indices as usize,
    );
    let vert_at = at + 24;
    let poly_at = vert_at + verts * 12;
    let index_at = poly_at + polys * 20;
    if index_at + indices > extra.len()
        || !read_v(extra, vert_at).array().iter().all(|x| x.is_finite())
    {
        return None;
    }
    let (min, max) = chunk_bounds(extra, vert_at, verts);
    Some(Chunk {
        vert_at,
        verts,
        edges,
        poly_at,
        polys,
        index_at,
        indices,
        min,
        max,
    })
}

fn near(a: V, b: V) -> bool {
    (a - b).len() < 0.25
}

fn normal(extra: &[u8], chunk: &Chunk, ids: &[usize]) -> Option<V> {
    let a = read_v(extra, chunk.vert_at + ids[0] * 12);
    for i in 1..ids.len() - 1 {
        let n = (read_v(extra, chunk.vert_at + ids[i] * 12) - a)
            .cross(read_v(extra, chunk.vert_at + ids[i + 1] * 12) - a);
        if n.dot(n) > 1e-8 {
            return Some(n.unit());
        }
    }
    None
}

fn recompute_planes(extra: &mut [u8], chunk: &Chunk) {
    for i in 0..chunk.polys {
        let o = chunk.poly_at + i * 20;
        let vref = usize::from(u16::from_le_bytes([extra[o + 16], extra[o + 17]]));
        let count = usize::from(extra[o + 18]);
        if vref + count > chunk.indices || count < 3 {
            continue;
        }
        let ids: Vec<usize> = (0..count)
            .map(|k| usize::from(extra[chunk.index_at + vref + k]))
            .collect();
        if ids.iter().any(|&id| id >= chunk.verts) {
            continue;
        }
        let Some(mut n) = normal(extra, chunk, &ids) else {
            continue;
        };
        if n.dot(read_v(extra, o)) < 0.0 {
            n = -n;
        }
        let p = read_v(extra, chunk.vert_at + ids[0] * 12);
        write_v(extra, o, n);
        write(extra, o + 12, -n.dot(p));
    }
}

fn write_bounds(extra: &mut [u8], chunk: &Chunk, now: (V, V)) -> Result<(), String> {
    let at = chunk.index_at + chunk.indices + chunk.edges * 2 + chunk.verts * 3 + 4;
    if at + 24 > extra.len()
        || i32_at(extra, at - 4) != 0
        || !near(read_v(extra, at), chunk.min)
        || !near(read_v(extra, at + 12), chunk.max)
    {
        return Err("convex cache bounds record not found".into());
    }
    write_v(extra, at, now.0);
    write_v(extra, at + 12, now.1);
    Ok(())
}

pub fn refresh(extra: &mut [u8], min: V, max: V, shear: f32) -> Result<usize, String> {
    if !shear.is_finite() {
        return Err("convex cache shear is not finite".into());
    }
    let mut updated = 0;
    let z_mid = (min.2 + max.2) * 0.5;
    let mut at = 0;
    while at + 24 < extra.len() {
        let Some(chunk) = (&extra[at..at + 4] == SIGNATURE)
            .then(|| valid(extra, at))
            .flatten()
        else {
            at += 1;
            continue;
        };
        let direct = near(chunk.min, min) && near(chunk.max, max);
        let mirrored =
            near(chunk.min, V(-max.0, min.1, -max.2)) && near(chunk.max, V(-min.0, max.1, -min.2));
        if direct && mirrored {
            return Err("convex cache orientation is ambiguous".into());
        }
        if direct || mirrored {
            if shear != 0.0 {
                for i in 0..chunk.verts {
                    let o = chunk.vert_at + i * 12;
                    let z = read(extra, o + 8);
                    let y = read(extra, o + 4) + shear * ((if mirrored { -z } else { z }) - z_mid);
                    write(extra, o + 4, y);
                }
            }
            recompute_planes(extra, &chunk);
            let now = chunk_bounds(extra, chunk.vert_at, chunk.verts);
            write_bounds(extra, &chunk, now)?;
            updated += 1;
        }
        at += 1;
    }
    Ok(updated)
}
