use super::{Corner, key};
use crate::mesh_math::V;
use std::collections::{HashMap, HashSet};

pub fn smooth(tris: Vec<[usize; 3]>, points: &[V]) -> Vec<Vec<Corner>> {
    let area: Vec<_> = tris
        .iter()
        .map(|f| (points[f[1]] - points[f[0]]).cross(points[f[2]] - points[f[0]]))
        .collect();
    let facing: Vec<_> = area.iter().map(|&v| v.unit()).collect();
    let mut around: HashMap<_, Vec<usize>> = HashMap::new();
    for (i, f) in tris.iter().enumerate() {
        for &p in f {
            around.entry(key(points[p])).or_default().push(i);
        }
    }
    let hard = (std::f32::consts::PI * 40.0 / 180.0).cos();
    tris.iter()
        .enumerate()
        .map(|(i, f)| {
            f.iter()
                .map(|&p| {
                    let mut sum = V::default();
                    let mut seen = HashSet::new();
                    for &h in &around[&key(points[p])] {
                        if seen.insert(h) && facing[h].dot(facing[i]) > hard {
                            sum = sum + area[h];
                        }
                    }
                    Corner {
                        point: p,
                        normal: sum.unit(),
                    }
                })
                .collect()
        })
        .collect()
}
