use super::decode::f32s;
use super::io::{Blob, Reader};
use super::skeleton_data::Skeleton;
use super::write::Writer;

pub struct ModelInfo {
    pub special: i32,
    pub bounding_sphere: f32,
    pub geometry_sphere: f32,
    pub remarks: i32,
    pub and_hints: i32,
    pub or_hints: i32,
    pub aiming_center: [f32; 3],
    pub color: u32,
    pub color_type: u32,
    pub view_density: f32,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    pub lod_density: f32,
    pub draw_importance: f32,
    pub bbox_min_visual: [f32; 3],
    pub bbox_max_visual: [f32; 3],
    pub bounding_center: [f32; 3],
    pub geometry_center: [f32; 3],
    pub center_of_mass: [f32; 3],
    pub inv_inertia: [[f32; 3]; 3],
    pub flags: [u8; 5],
    pub ht_min: f32,
    pub ht_max: f32,
    pub af_max: f32,
    pub mf_max: f32,
    pub m_fact: f32,
    pub t_body: f32,
    pub force_not_alpha: u8,
    pub sb_source: i32,
    pub prefer_shadow_volume: u8,
    pub shadow_offset: f32,
    pub animated: u8,
    pub skeleton: Skeleton,
    pub map_type: u8,
    pub mass: Vec<f32>,
    pub mass_origin: Blob,
    pub total_mass: f32,
    pub inv_mass: f32,
    pub armor: f32,
    pub inv_armor: f32,
    pub explosion_shielding: f32,
    pub geometry_simple: u8,
    pub geometry_phys: u8,
    pub lod_index: [u8; 12],
    pub min_shadow: u32,
    pub can_blend: u8,
    pub class: Vec<u8>,
    pub damage: Vec<u8>,
    pub frequent: u8,
    pub unused31: u32,
    pub preferred_shadow_volume: Vec<i32>,
    pub preferred_shadow_buffer: Vec<i32>,
    pub preferred_shadow_buffer_vis: Vec<i32>,
}

impl ModelInfo {
    pub fn memory(&self) -> u8 {
        self.lod_index[0]
    }

    pub fn read(r: &mut Reader<'_>, lods: usize) -> Result<Self, String> {
        let special = r.i32()?;
        let bounding_sphere = r.f32()?;
        let geometry_sphere = r.f32()?;
        let remarks = r.i32()?;
        let and_hints = r.i32()?;
        let or_hints = r.i32()?;
        let aiming_center = r.vec3()?;
        let color = r.u32()?;
        let color_type = r.u32()?;
        let view_density = r.f32()?;
        let bbox_min = r.vec3()?;
        let bbox_max = r.vec3()?;
        let lod_density = r.f32()?;
        let draw_importance = r.f32()?;
        let bbox_min_visual = r.vec3()?;
        let bbox_max_visual = r.vec3()?;
        let bounding_center = r.vec3()?;
        let geometry_center = r.vec3()?;
        let center_of_mass = r.vec3()?;
        let inv_inertia = [r.vec3()?, r.vec3()?, r.vec3()?];
        let mut flags = [0; 5];
        for flag in &mut flags {
            *flag = r.u8()?;
        }
        let ht_min = r.f32()?;
        let ht_max = r.f32()?;
        let af_max = r.f32()?;
        let mf_max = r.f32()?;
        let m_fact = r.f32()?;
        let t_body = r.f32()?;
        let force_not_alpha = r.u8()?;
        let sb_source = r.i32()?;
        let prefer_shadow_volume = r.u8()?;
        let shadow_offset = r.f32()?;
        let animated = r.u8()?;
        let skeleton = Skeleton::read(r)?;
        let map_type = r.u8()?;
        let (mass_origin, mass_raw) = r.packed_array(4)?;
        let mut lod_index = [0; 12];
        let mass = f32s(&mass_raw)?;
        let total_mass = r.f32()?;
        let inv_mass = r.f32()?;
        let armor = r.f32()?;
        let inv_armor = r.f32()?;
        let explosion_shielding = r.f32()?;
        let geometry_simple = r.u8()?;
        let geometry_phys = r.u8()?;
        for byte in &mut lod_index {
            *byte = r.u8()?;
        }
        Ok(Self {
            special,
            bounding_sphere,
            geometry_sphere,
            remarks,
            and_hints,
            or_hints,
            aiming_center,
            color,
            color_type,
            view_density,
            bbox_min,
            bbox_max,
            lod_density,
            draw_importance,
            bbox_min_visual,
            bbox_max_visual,
            bounding_center,
            geometry_center,
            center_of_mass,
            inv_inertia,
            flags,
            ht_min,
            ht_max,
            af_max,
            mf_max,
            m_fact,
            t_body,
            force_not_alpha,
            sb_source,
            prefer_shadow_volume,
            shadow_offset,
            animated,
            skeleton,
            map_type,
            mass,
            mass_origin,
            total_mass,
            inv_mass,
            armor,
            inv_armor,
            explosion_shielding,
            geometry_simple,
            geometry_phys,
            lod_index,
            min_shadow: r.u32()?,
            can_blend: r.u8()?,
            class: r.asciiz()?,
            damage: r.asciiz()?,
            frequent: r.u8()?,
            unused31: r.u32()?,
            preferred_shadow_volume: r.i32s(lods)?,
            preferred_shadow_buffer: r.i32s(lods)?,
            preferred_shadow_buffer_vis: r.i32s(lods)?,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.i32(self.special);
        w.f32(self.bounding_sphere);
        w.f32(self.geometry_sphere);
        w.i32(self.remarks);
        w.i32(self.and_hints);
        w.i32(self.or_hints);
        w.vec3(&self.aiming_center);
        w.u32(self.color);
        w.u32(self.color_type);
        w.f32(self.view_density);
        w.vec3(&self.bbox_min);
        w.vec3(&self.bbox_max);
        w.f32(self.lod_density);
        w.f32(self.draw_importance);
        w.vec3(&self.bbox_min_visual);
        w.vec3(&self.bbox_max_visual);
        w.vec3(&self.bounding_center);
        w.vec3(&self.geometry_center);
        w.vec3(&self.center_of_mass);
        for row in &self.inv_inertia {
            w.vec3(row);
        }
        for flag in self.flags {
            w.u8(flag);
        }
        w.f32(self.ht_min);
        w.f32(self.ht_max);
        w.f32(self.af_max);
        w.f32(self.mf_max);
        w.f32(self.m_fact);
        w.f32(self.t_body);
        w.u8(self.force_not_alpha);
        w.i32(self.sb_source);
        w.u8(self.prefer_shadow_volume);
        w.f32(self.shadow_offset);
        w.u8(self.animated);
        self.skeleton.write(w)?;
        w.u8(self.map_type);
        w.blob(&self.mass_origin);
        w.f32(self.total_mass);
        w.f32(self.inv_mass);
        w.f32(self.armor);
        w.f32(self.inv_armor);
        w.f32(self.explosion_shielding);
        w.u8(self.geometry_simple);
        w.u8(self.geometry_phys);
        for byte in self.lod_index {
            w.u8(byte);
        }
        w.u32(self.min_shadow);
        w.u8(self.can_blend);
        w.asciiz(&self.class);
        w.asciiz(&self.damage);
        w.u8(self.frequent);
        w.u32(self.unused31);
        for value in &self.preferred_shadow_volume {
            w.i32(*value);
        }
        for value in &self.preferred_shadow_buffer {
            w.i32(*value);
        }
        for value in &self.preferred_shadow_buffer_vis {
            w.i32(*value);
        }
        Ok(())
    }
}
