use std::{env, fs};
use uksf_patcher::{odol::Model, pbo::Pbo};

fn main() {
    let args: Vec<String> = env::args().collect();
    let pbo = Pbo::parse(fs::read(&args[1]).unwrap()).unwrap();
    let index = pbo.find(args[2].as_bytes()).unwrap().unwrap();
    let model = Model::parse(&pbo.data(index).unwrap()).unwrap();
    let b: Vec<f32> = args[3].split(',').map(|s| s.parse().unwrap()).collect();
    let lod = &model.lods[0];
    for (i, v) in lod.vertices.iter().enumerate() {
        if v[0] >= b[0] && v[0] <= b[1] && v[1] >= b[2] && v[1] <= b[3] && v[2] >= b[4] && v[2] <= b[5] {
            println!("{i} {} {} {}", v[0], v[1], v[2]);
        }
    }
    for (fi, f) in lod.faces.iter().enumerate() {
        let inside = f.iter().all(|&v| {
            let p = lod.vertices[v as usize];
            p[0] >= b[0] && p[0] <= b[1] && p[1] >= b[2] && p[1] <= b[3] && p[2] >= b[4] && p[2] <= b[5]
        });
        if inside {
            println!("f{fi} {:?}", f);
        }
    }
}
