use super::io::Reader;
use super::write::Writer;

#[derive(Debug, PartialEq)]
pub struct Bone {
    pub name: Vec<u8>,
    pub parent: Vec<u8>,
}

pub struct Skeleton {
    pub name: Vec<u8>,
    pub discrete: u8,
    pub bones: Vec<Bone>,
    pub pivots: Vec<u8>,
}

impl Skeleton {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let name = r.asciiz()?;
        if name.is_empty() {
            return Ok(Self {
                name,
                discrete: 0,
                bones: Vec::new(),
                pivots: Vec::new(),
            });
        }
        let discrete = r.u8()?;
        let count = r.count()?;
        r.limit(count, 2)?;
        let mut bones = Vec::with_capacity(count);
        for _ in 0..count {
            bones.push(Bone {
                name: r.asciiz()?,
                parent: r.asciiz()?,
            });
        }
        Ok(Self {
            name,
            discrete,
            bones,
            pivots: r.asciiz()?,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.asciiz(&self.name);
        if self.name.is_empty() {
            return Ok(());
        }
        w.u8(self.discrete);
        w.count(self.bones.len())?;
        for bone in &self.bones {
            w.asciiz(&bone.name);
            w.asciiz(&bone.parent);
        }
        w.asciiz(&self.pivots);
        Ok(())
    }
}
