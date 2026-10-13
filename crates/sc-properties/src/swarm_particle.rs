//! Decode validated Swarm sprite-particle components used by visual effects.

use serde::Serialize;

use crate::swarm::{SwarmCollection, SwarmError};

const PARTICLE: u16 = 1;
const META_PARTICLE: u16 = 2;
const MAX_CURVE_KEYS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureRef {
    pub group: u32,
    pub instance: u32,
    pub draw_mode: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleComponent {
    pub type_code: u16,
    pub index: usize,
    pub flags: u64,
    pub life: [f32; 2],
    pub emit_direction_min: [f32; 3],
    pub emit_direction_max: [f32; 3],
    pub emit_speed: [f32; 2],
    pub emit_volume_min: [f32; 3],
    pub emit_volume_max: [f32; 3],
    pub rate: Vec<f32>,
    pub rate_loop: f32,
    pub size: Vec<f32>,
    pub alpha: Vec<f32>,
    pub color: Vec<[f32; 3]>,
    pub texture: TextureRef,
    pub direction_force: [f32; 3],
    pub wind_strength: f32,
    pub gravity_strength: f32,
    pub drag: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentRef {
    pub type_code: u16,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaParticleComponent {
    pub index: usize,
    pub emitter: ParticleComponent,
    pub child: Option<ComponentRef>,
}

pub fn decode_meta_particle_components(
    collection: &SwarmCollection,
    data: &[u8],
) -> Result<Vec<MetaParticleComponent>, SwarmError> {
    let Some(section) = collection.component_section(META_PARTICLE) else {
        return Ok(Vec::new());
    };
    let end = section
        .payload_offset
        .checked_add(section.payload_size)
        .ok_or(SwarmError::Truncated(section.payload_offset))?;
    let payload = data
        .get(section.payload_offset..end)
        .ok_or(SwarmError::Truncated(section.payload_offset))?;
    let mut r = Reader {
        data: payload,
        pos: 0,
        base: section.payload_offset,
        little_arrays: section.version == 1,
    };
    let mut out = Vec::with_capacity(section.count as usize);
    for index in 0..section.count as usize {
        let flags = (r.u32()? as u64) << 32 | r.u32()? as u64;
        let mut emitter = read_common(&mut r, flags, true, true)?;
        emitter.type_code = META_PARTICLE;
        emitter.index = index;
        emitter.size = r.f32_curve()?;
        r.skip(4)?; // size variation
        for _ in 0..3 {
            r.skip_curve(4)?;
        } // pitch, roll, yaw
        r.skip(24)?; // variation and offset for three axes
        emitter.color = r.color_curve()?;
        r.skip(12)?; // color variation
        emitter.alpha = r.f32_curve()?;
        r.skip(4)?; // alpha variation
        let child_index = r.u32()? as i32;
        let child_type = r.u32()?;
        r.skip(8)?; // death effect reference
        let child = if child_index >= 0 && child_type <= u16::MAX as u32 {
            Some(ComponentRef {
                type_code: child_type as u16,
                index: child_index as usize,
            })
        } else {
            None
        };
        r.skip(1)?; // alignment
        emitter.direction_force = r.f32_triple_le()?;
        r.skip(12)?; // global forces
        emitter.wind_strength = r.f32()?;
        emitter.gravity_strength = r.f32()?;
        r.skip(4 + 12)?; // radial force and location
        emitter.drag = r.f32()?;
        r.skip_curve(28)?; // wiggles
        r.skip(4)?; // bloom controls
        r.skip_curve(12)?;
        r.skip_curve(4)?;
        let surface_count = r.count()?;
        for _ in 0..surface_count {
            r.skip(40)?;
            r.skip_curve(12)?;
        }
        r.skip(28 + 8 + 24)?; // map physics, altitude and resources
        for _ in 0..2 {
            r.skip(28)?; // random walk scalars
            r.skip_curve(4)?;
            r.skip(1)?;
        }
        r.skip(12 + 12 + 12)?; // preferred direction, alignment and attractor origin
        r.skip_curve(4)?;
        r.skip(8)?;
        r.skip_curve(28)?;
        r.skip(4)?; // tractor reset speed
        out.push(MetaParticleComponent {
            index,
            emitter,
            child,
        });
    }
    if r.pos != payload.len() {
        return Err(SwarmError::Invalid(
            r.base + r.pos,
            "meta particle section size mismatch",
        ));
    }
    Ok(out)
}

pub fn decode_particle_components(
    collection: &SwarmCollection,
    data: &[u8],
) -> Result<Vec<ParticleComponent>, SwarmError> {
    let (components, error) = decode_particle_components_prefix(collection, data);
    match error {
        Some(error) => Err(error),
        None => Ok(components),
    }
}

pub fn decode_particle_components_prefix(
    collection: &SwarmCollection,
    data: &[u8],
) -> (Vec<ParticleComponent>, Option<SwarmError>) {
    let mut out = Vec::new();
    for section in &collection.components {
        if section.type_code != PARTICLE {
            continue;
        }
        if !(1..=2).contains(&section.version) {
            return (
                out,
                Some(SwarmError::Invalid(
                    section.payload_offset,
                    "unsupported particle version",
                )),
            );
        }
        let Some(end) = section.payload_offset.checked_add(section.payload_size) else {
            return (out, Some(SwarmError::Truncated(section.payload_offset)));
        };
        let Some(payload) = data.get(section.payload_offset..end) else {
            return (out, Some(SwarmError::Truncated(section.payload_offset)));
        };
        let mut r = Reader {
            data: payload,
            pos: 0,
            base: section.payload_offset,
            little_arrays: section.version == 1,
        };
        for index in 0..section.count as usize {
            let result = read_particle(&mut r);
            let mut component = match result {
                Ok(component) => component,
                Err(error) => return (out, Some(error)),
            };
            component.type_code = section.type_code;
            component.index = index;
            out.push(component);
        }
        if r.pos != payload.len() {
            return (
                out,
                Some(SwarmError::Invalid(
                    r.base + r.pos,
                    "particle section size mismatch",
                )),
            );
        }
    }
    (out, None)
}

fn read_common(
    r: &mut Reader<'_>,
    flags: u64,
    has_torus: bool,
    meta: bool,
) -> Result<ParticleComponent, SwarmError> {
    let life = r.f32_pair_le()?;
    r.skip(4 + 8 + 8)?; // preroll, emit delay, retrigger
    let emit_direction_min = r.f32_triple_le()?;
    let emit_direction_max = r.f32_triple_le()?;
    let emit_speed = r.f32_pair_le()?;
    let emit_volume_min = r.f32_triple_le()?;
    let emit_volume_max = r.f32_triple_le()?;
    if has_torus {
        r.skip(4)?;
    }
    let rate = r.f32_curve()?;
    let rate_loop = r.f32()?;
    if meta {
        r.skip(4)?;
    }
    // MetaParticle cycles
    else {
        if r.little_arrays {
            r.skip(6)?;
        } else {
            let extended = r.plausible_curve_count(14);
            let compact = r.plausible_curve_count(6);
            if extended {
                r.skip(14)?;
            } else if compact {
                r.skip(6)?;
            } else {
                return Err(SwarmError::Invalid(
                    r.base + r.pos,
                    "unknown particle rate layout",
                ));
            }
        }
    }
    Ok(ParticleComponent {
        type_code: 0,
        index: 0,
        flags,
        life,
        emit_direction_min,
        emit_direction_max,
        emit_speed,
        emit_volume_min,
        emit_volume_max,
        rate,
        rate_loop,
        size: Vec::new(),
        alpha: Vec::new(),
        color: Vec::new(),
        texture: TextureRef {
            group: u32::MAX,
            instance: u32::MAX,
            draw_mode: 15,
        },
        direction_force: [0.0; 3],
        wind_strength: 0.0,
        gravity_strength: 0.0,
        drag: 0.0,
    })
}

fn read_texture(r: &mut Reader<'_>) -> Result<TextureRef, SwarmError> {
    let group = r.u32()?;
    let instance = r.u32()?;
    r.skip(1)?; // format
    let draw_mode = r.u8()?;
    r.skip(2 + 2 + 4 + 8)?; // flags, buffer, layer, sort offset, second resource
    Ok(TextureRef {
        group,
        instance,
        draw_mode,
    })
}

fn read_particle(r: &mut Reader<'_>) -> Result<ParticleComponent, SwarmError> {
    let high = r.u32()? as u64;
    let low = r.u32()? as u64;
    let mut p = read_common(r, (high << 32) | low, true, false)?;
    p.size = r.f32_curve()?;
    r.skip(4)?; // size variation
    r.skip_curve(4)?; // aspect ratio
    r.skip(12)?; // aspect variation, rotation variation/offset
    r.skip_curve(4)?; // rotation curve
    p.alpha = r.f32_curve()?;
    r.skip(4)?; // alpha variation
    p.color = r.color_curve()?;
    r.skip(12)?; // color variation
    p.texture = read_texture(r)?;
    r.skip(8 + 4)?; // physics, tiling, frame controls, frame speed
    p.direction_force = r.f32_triple_le()?;
    p.wind_strength = r.f32()?;
    p.gravity_strength = r.f32()?;
    r.skip(4 + 12)?; // radial force and location
    p.drag = r.f32()?;
    r.skip(8)?; // stretch, screw rate
    r.skip_curve(28)?; // wiggles
    r.skip(4)?; // bloom controls
    r.skip_curve(12)?; // loop box colors
    r.skip_curve(4)?; // loop box alpha
    let surface_count = r.count()?;
    for _ in 0..surface_count {
        r.skip(40)?; // surface flags, map, physics and child effect references
        r.skip_curve(12)?; // surface points
    }
    r.skip(28 + 8 + 24)?; // map physics, altitude, three map resource IDs
    r.skip(8 + 8 + 12)?; // random walk time/strength/range
    r.skip_curve(4)?; // random walk curve
    r.skip(1 + 12)?; // random walk loop type and attractor origin
    r.skip_curve(4)?; // attractor strength
    r.skip(8)?; // attractor range and kill range
    r.skip_curve(28)?; // path points
    Ok(p)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    base: usize,
    little_arrays: bool,
}

impl Reader<'_> {
    fn plausible_curve_count(&self, offset: usize) -> bool {
        self.data
            .get(self.pos + offset..self.pos + offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_be_bytes)
            .is_some_and(|count| (1..=1024).contains(&count))
    }
    fn take(&mut self, len: usize) -> Result<&[u8], SwarmError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(SwarmError::Truncated(self.base + self.pos))?;
        let bytes = self
            .data
            .get(self.pos..end)
            .ok_or(SwarmError::Truncated(self.base + self.pos))?;
        self.pos = end;
        Ok(bytes)
    }
    fn skip(&mut self, len: usize) -> Result<(), SwarmError> {
        self.take(len).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, SwarmError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, SwarmError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, SwarmError> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn f32_le(&mut self) -> Result<f32, SwarmError> {
        if self.little_arrays {
            Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
        } else {
            self.f32()
        }
    }
    fn f32_pair_le(&mut self) -> Result<[f32; 2], SwarmError> {
        Ok([self.f32_le()?, self.f32_le()?])
    }
    fn f32_triple_le(&mut self) -> Result<[f32; 3], SwarmError> {
        Ok([self.f32_le()?, self.f32_le()?, self.f32_le()?])
    }
    fn count(&mut self) -> Result<usize, SwarmError> {
        let at = self.base + self.pos;
        let count = self.u32()? as usize;
        if count > MAX_CURVE_KEYS {
            return Err(SwarmError::Invalid(at, "particle list too large"));
        }
        Ok(count)
    }
    fn skip_curve(&mut self, stride: usize) -> Result<(), SwarmError> {
        let count = self.count()?;
        self.skip(
            count
                .checked_mul(stride)
                .ok_or(SwarmError::Truncated(self.base + self.pos))?,
        )
    }
    fn f32_curve(&mut self) -> Result<Vec<f32>, SwarmError> {
        let count = self.count()?;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.f32()?);
        }
        Ok(out)
    }
    fn color_curve(&mut self) -> Result<Vec<[f32; 3]>, SwarmError> {
        let count = self.count()?;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.f32_triple_le()?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_collection_particle_sections_align() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tmp/gltf_fixture/effects_main.col"
        );
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let collection = SwarmCollection::parse(&bytes).unwrap();
        let particles = decode_particle_components(&collection, &bytes).unwrap();
        assert_eq!(particles.len(), 37);
        assert!(particles.iter().any(|p| p.type_code == PARTICLE));
        assert!(
            particles
                .iter()
                .all(|p| p.life.iter().all(|v| v.is_finite()))
        );
        assert_eq!(particles[0].life, [3.0, 3.0]);
        assert_eq!(particles[0].size, vec![1.0, 1.0, 1.0, 1.2, 1.5, 1.8, 2.0]);
        assert_eq!(particles[0].texture.instance, 0x4D27_2BC5);
    }

    #[test]
    fn retail_meta_particle_section_aligns() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tmp/gltf_fixture/effects_main.col"
        );
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let collection = SwarmCollection::parse(&bytes).unwrap();
        let meta = decode_meta_particle_components(&collection, &bytes).unwrap();
        assert_eq!(meta.len(), 250);
        assert!(
            meta.iter()
                .all(|item| item.emitter.life.iter().all(|v| v.is_finite()))
        );
        assert!(meta.iter().any(|item| {
            item.child
                .as_ref()
                .is_some_and(|child| child.type_code == 32)
        }));
    }
}
