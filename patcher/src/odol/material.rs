use super::io::Reader;
use super::write::Writer;

pub struct StageTexture {
    pub filter: u32,
    pub texture: Vec<u8>,
    pub stage_id: u32,
    pub world_env: u8,
    pub has_filter: bool,
    pub has_stage: bool,
    pub has_world: bool,
}

pub struct StageTransform {
    pub uv_source: u32,
    pub matrix: [f32; 12],
}

pub struct Material {
    pub name: Vec<u8>,
    pub version: u32,
    pub colors: [[f32; 4]; 6],
    pub specular_power: f32,
    pub pixel_shader: u32,
    pub vertex_shader: u32,
    pub main_light: u32,
    pub fog_mode: u32,
    pub unused3: u8,
    pub surface: Vec<u8>,
    pub render_flags: u32,
    pub render_value: u32,
    pub textures: Vec<StageTexture>,
    pub transforms: Vec<StageTransform>,
    pub stage_ti: Option<StageTexture>,
}

impl StageTexture {
    fn read(r: &mut Reader<'_>, version: u32) -> Result<Self, String> {
        let has_filter = version >= 5;
        let filter = if has_filter { r.u32()? } else { 0 };
        let texture = r.asciiz()?;
        let has_stage = version >= 8;
        let stage_id = if has_stage { r.u32()? } else { 0 };
        let has_world = version >= 11;
        let world_env = if has_world { r.u8()? } else { 0 };
        Ok(Self {
            filter,
            texture,
            stage_id,
            world_env,
            has_filter,
            has_stage,
            has_world,
        })
    }

    fn write(&self, w: &mut Writer) {
        if self.has_filter {
            w.u32(self.filter);
        }
        w.asciiz(&self.texture);
        if self.has_stage {
            w.u32(self.stage_id);
        }
        if self.has_world {
            w.u8(self.world_env);
        }
    }
}

impl StageTransform {
    fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let uv_source = r.u32()?;
        let mut matrix = [0.0; 12];
        for value in &mut matrix {
            *value = r.f32()?;
        }
        Ok(Self { uv_source, matrix })
    }

    fn write(&self, w: &mut Writer) {
        w.u32(self.uv_source);
        for value in self.matrix {
            w.f32(value);
        }
    }
}

fn stage_bytes(version: u32) -> usize {
    let mut n = 1;
    if version >= 5 {
        n += 4;
    }
    if version >= 8 {
        n += 4;
    }
    if version >= 11 {
        n += 1;
    }
    n
}

impl Material {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let name = r.asciiz()?;
        let version = r.u32()?;
        let mut colors = [[0.0; 4]; 6];
        for color in &mut colors {
            for channel in color.iter_mut() {
                *channel = r.f32()?;
            }
        }
        let specular_power = r.f32()?;
        let pixel_shader = r.u32()?;
        let vertex_shader = r.u32()?;
        let main_light = r.u32()?;
        let fog_mode = r.u32()?;
        let unused3 = if version == 3 { r.u8()? } else { 0 };
        let surface = if version >= 6 {
            r.asciiz()?
        } else {
            Vec::new()
        };
        let (render_flags, render_value) = if version >= 4 {
            (r.u32()?, r.u32()?)
        } else {
            (0, 0)
        };
        let texture_count = if version > 6 { r.u32()? as usize } else { 0 };
        let transform_count = if version > 8 {
            r.u32()? as usize
        } else {
            texture_count
        };
        let tex_min = stage_bytes(version);
        if version < 8 {
            r.limit(texture_count, 52 + tex_min)?;
        } else {
            r.limit(texture_count, tex_min)?;
            r.limit(transform_count, 52)?;
        }
        let mut textures = Vec::with_capacity(texture_count);
        let mut transforms = Vec::with_capacity(transform_count);
        if version < 8 {
            for _ in 0..texture_count {
                transforms.push(StageTransform::read(r)?);
                textures.push(StageTexture::read(r, version)?);
            }
        } else {
            for _ in 0..texture_count {
                textures.push(StageTexture::read(r, version)?);
            }
            for _ in 0..transform_count {
                transforms.push(StageTransform::read(r)?);
            }
        }
        let stage_ti = if version >= 10 {
            Some(StageTexture::read(r, version)?)
        } else {
            None
        };
        Ok(Self {
            name,
            version,
            colors,
            specular_power,
            pixel_shader,
            vertex_shader,
            main_light,
            fog_mode,
            unused3,
            surface,
            render_flags,
            render_value,
            textures,
            transforms,
            stage_ti,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.asciiz(&self.name);
        w.u32(self.version);
        for color in &self.colors {
            for channel in color {
                w.f32(*channel);
            }
        }
        w.f32(self.specular_power);
        w.u32(self.pixel_shader);
        w.u32(self.vertex_shader);
        w.u32(self.main_light);
        w.u32(self.fog_mode);
        if self.version == 3 {
            w.u8(self.unused3);
        }
        if self.version >= 6 {
            w.asciiz(&self.surface);
        }
        if self.version >= 4 {
            w.u32(self.render_flags);
            w.u32(self.render_value);
        }
        if self.version > 6 {
            w.u32(u32::try_from(self.textures.len()).map_err(|_| "material stage overflow")?);
        }
        if self.version > 8 {
            w.u32(u32::try_from(self.transforms.len()).map_err(|_| "material stage overflow")?);
        }
        if self.version < 8 {
            for (transform, texture) in self.transforms.iter().zip(&self.textures) {
                transform.write(w);
                texture.write(w);
            }
        } else {
            for texture in &self.textures {
                texture.write(w);
            }
            for transform in &self.transforms {
                transform.write(w);
            }
        }
        if let Some(stage) = &self.stage_ti {
            stage.write(w);
        }
        Ok(())
    }
}
