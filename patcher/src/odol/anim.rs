use super::io::Reader;
use super::write::Writer;

pub struct Animation {
    pub kind: u32,
    pub name: Vec<u8>,
    pub source: Vec<u8>,
    pub min_phase: f32,
    pub max_phase: f32,
    pub min_value: f32,
    pub max_value: f32,
    pub period: f32,
    pub init_phase: f32,
    pub source_address: u32,
    pub angle0: f32,
    pub angle1: f32,
    pub offset0: f32,
    pub offset1: f32,
    pub axis_pos: [f32; 3],
    pub axis_dir: [f32; 3],
    pub angle: f32,
    pub axis_offset: f32,
    pub hide: f32,
    pub unused55: f32,
}

pub struct Animations {
    pub classes: Vec<Animation>,
    pub bones_to_anims: Vec<Vec<Vec<u32>>>,
    pub anims_to_bones: Vec<Vec<i32>>,
    pub axes: Vec<Vec<Option<[[f32; 3]; 2]>>>,
}

impl Animation {
    fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let kind = r.u32()?;
        let mut anim = Self {
            kind,
            name: r.asciiz()?,
            source: r.asciiz()?,
            min_phase: r.f32()?,
            max_phase: r.f32()?,
            min_value: r.f32()?,
            max_value: r.f32()?,
            period: r.f32()?,
            init_phase: r.f32()?,
            source_address: r.u32()?,
            angle0: 0.0,
            angle1: 0.0,
            offset0: 0.0,
            offset1: 0.0,
            axis_pos: [0.0; 3],
            axis_dir: [0.0; 3],
            angle: 0.0,
            axis_offset: 0.0,
            hide: 0.0,
            unused55: 0.0,
        };
        match kind {
            0..=3 => {
                anim.angle0 = r.f32()?;
                anim.angle1 = r.f32()?;
            }
            4..=7 => {
                anim.offset0 = r.f32()?;
                anim.offset1 = r.f32()?;
            }
            8 => {
                anim.axis_pos = r.vec3()?;
                anim.axis_dir = r.vec3()?;
                anim.angle = r.f32()?;
                anim.axis_offset = r.f32()?;
            }
            9 => {
                anim.hide = r.f32()?;
                anim.unused55 = r.f32()?;
            }
            _ => return Err(r.err(&format!("unknown ODOL animation type {kind}"))),
        }
        Ok(anim)
    }

    fn write(&self, w: &mut Writer) {
        w.u32(self.kind);
        w.asciiz(&self.name);
        w.asciiz(&self.source);
        w.f32(self.min_phase);
        w.f32(self.max_phase);
        w.f32(self.min_value);
        w.f32(self.max_value);
        w.f32(self.period);
        w.f32(self.init_phase);
        w.u32(self.source_address);
        match self.kind {
            0..=3 => {
                w.f32(self.angle0);
                w.f32(self.angle1);
            }
            4..=7 => {
                w.f32(self.offset0);
                w.f32(self.offset1);
            }
            8 => {
                w.vec3(&self.axis_pos);
                w.vec3(&self.axis_dir);
                w.f32(self.angle);
                w.f32(self.axis_offset);
            }
            9 => {
                w.f32(self.hide);
                w.f32(self.unused55);
            }
            _ => {}
        }
    }
}

impl Animations {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let class_count = r.count()?;
        r.limit(class_count, 38)?;
        let mut classes = Vec::with_capacity(class_count);
        for _ in 0..class_count {
            classes.push(Animation::read(r)?);
        }
        let bone_count = r.count()?;
        r.limit(bone_count, 4)?;
        let mut bones_to_anims = Vec::with_capacity(bone_count);
        for _ in 0..bone_count {
            let outer = r.count()?;
            r.limit(outer, 4)?;
            let mut rows = Vec::with_capacity(outer);
            for _ in 0..outer {
                let inner = r.count()?;
                rows.push(r.u32s(inner)?);
            }
            bones_to_anims.push(rows);
        }
        r.limit(bone_count, classes.len().saturating_mul(4))?;
        let mut anims_to_bones = Vec::with_capacity(bone_count);
        let mut axes = Vec::with_capacity(bone_count);
        for _ in 0..bone_count {
            r.limit(classes.len(), 4)?;
            let mut row = Vec::with_capacity(classes.len());
            let mut axis_row = Vec::with_capacity(classes.len());
            for class in &classes {
                let link = r.i32()?;
                let axis = if link != -1 && class.kind != 8 && class.kind != 9 {
                    Some([r.vec3()?, r.vec3()?])
                } else {
                    None
                };
                row.push(link);
                axis_row.push(axis);
            }
            anims_to_bones.push(row);
            axes.push(axis_row);
        }
        Ok(Self {
            classes,
            bones_to_anims,
            anims_to_bones,
            axes,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.count(self.classes.len())?;
        for class in &self.classes {
            class.write(w);
        }
        w.count(self.bones_to_anims.len())?;
        for bone in &self.bones_to_anims {
            w.count(bone.len())?;
            for row in bone {
                w.count(row.len())?;
                for value in row {
                    w.u32(*value);
                }
            }
        }
        for (links, axes) in self.anims_to_bones.iter().zip(&self.axes) {
            for (index, link) in links.iter().enumerate() {
                w.i32(*link);
                if let Some(axis) = axes.get(index).and_then(|axis| axis.as_ref()) {
                    w.vec3(&axis[0]);
                    w.vec3(&axis[1]);
                }
            }
        }
        Ok(())
    }
}
