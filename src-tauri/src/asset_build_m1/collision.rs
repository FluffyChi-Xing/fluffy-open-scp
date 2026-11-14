//! Native collision proxy parsing and positive affine transformation.
//!
//! Port of the author's `collision_proxy.py`: preserves a known box's
//! triangulation, attributes and KD tree, and rescales it to the compiled
//! model's bounds. It does not invent or regenerate the undocumented
//! collision-tree encoding for arbitrary topology — the source template
//! (`collision-source.rw4`) must remain a verified native six-face box.

use rw4::{Rw4File, SectionType};

const FLAT: usize = 4; // f32/int bytes per scalar
const BOX_TRIANGLES: u32 = 12;
const BOX_VERTICES: u32 = 24;

fn align(n: u32, a: u32) -> Option<u32> {
    debug_assert!(
        a > 0 && a & (a - 1) == 0,
        "alignment must be a power of two"
    );
    Some(n.checked_add(a - 1)? & !(a - 1))
}

fn read_u32(payload: &[u8], at: usize) -> Option<u32> {
    payload
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn read_f32(payload: &[u8], at: usize) -> Option<f32> {
    payload
        .get(at..at + 4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
}

fn read_i32(payload: &[u8], at: usize) -> Option<i32> {
    payload
        .get(at..at + 4)
        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
}

/// One parsed native collision section (six-face box with its KD tree).
#[derive(Debug, Clone)]
pub struct Collision {
    /// Original section bytes. The author patches this buffer in place so
    /// vertex padding, triangle flags, attributes and tree tags survive.
    payload: Vec<u8>,
    pub low: [f32; 3],
    pub high: [f32; 3],
    /// Opaque header words carried through verbatim (magic, depth).
    magic: u32,
    depth: u32,
    pub triangles: Vec<[u32; 3]>,
    pub vertices: Vec<[f32; 3]>,
    /// `(tag, axis, left_count, left_start, right_count, right_start, split_lo, split_hi)`.
    pub tree: Vec<(u32, u32, i32, i32, i32, i32, f32, f32)>,
    /// Section layout offsets relative to the payload.
    pt: u32,
    pv: u32,
    p4: u32,
    p3: u32,
}

impl Collision {
    /// Parse and validate one `0x80003` payload. `base` is the section's
    /// absolute position inside its RW4 container; embedded self references
    /// are translated relative to it.
    pub fn parse(payload: &[u8], base: u32) -> Result<Self, String> {
        if payload.len() < 116 {
            return Err("truncated collision".into());
        }
        let mut low = [0f32; 3];
        let mut high = [0f32; 3];
        for axis in 0..3 {
            low[axis] = read_f32(payload, axis * FLAT).ok_or("truncated collision bounds")?;
            high[axis] = read_f32(payload, 16 + axis * FLAT).ok_or("truncated collision bounds")?;
            if !low[axis].is_finite() || !high[axis].is_finite() {
                return Err("non-finite collision bounds".into());
            }
        }
        let magic = read_u32(payload, 32).ok_or("truncated collision header")?;
        let depth = read_u32(payload, 36).ok_or("truncated collision header")?;
        let nt = read_u32(payload, 40).ok_or("truncated collision header")?;
        let zero = read_u32(payload, 44).ok_or("truncated collision header")?;
        let nv = read_u32(payload, 48).ok_or("truncated collision header")?;
        let pt_abs = read_u32(payload, 52).ok_or("truncated collision header")?;
        let pv_abs = read_u32(payload, 56).ok_or("truncated collision header")?;
        let p4_abs = read_u32(payload, 60).ok_or("truncated collision header")?;
        let p3_abs = read_u32(payload, 64).ok_or("truncated collision header")?;
        if magic == 0 {
            return Err("invalid collision magic".into());
        }
        if zero != 0 || nv == 0 || nv > 1_000_000 || nt == 0 || nt > 1_000_000 {
            return Err("invalid collision counts".into());
        }
        let pv = pv_abs
            .checked_sub(base)
            .ok_or("unsupported collision layout")?;
        let pt = pt_abs
            .checked_sub(base)
            .ok_or("unsupported collision layout")?;
        let p4 = p4_abs
            .checked_sub(base)
            .ok_or("unsupported collision layout")?;
        let p3 = p3_abs
            .checked_sub(base)
            .ok_or("unsupported collision layout")?;
        let pv_expect = align(base + 68, 16)
            .and_then(|v| v.checked_sub(base))
            .ok_or("layout")?;
        if pv != pv_expect || pt != pv + nv * 16 || p3 != pt + nt * 16 {
            return Err("unsupported collision layout".into());
        }
        // p4/p3 are payload-relative already, so no base subtraction here.
        if align(p3 + (nt + 7) / 8 * 4, 16) != Some(p4) {
            return Err("unsupported collision attribute layout".into());
        }
        let tail = (p4 + 48) as usize;
        if tail > payload.len() {
            return Err("truncated collision tail".into());
        }
        let anchor = read_u32(payload, p4 as usize).ok_or("truncated collision tail")?;
        let nodes = read_u32(payload, p4 as usize + 4).ok_or("truncated collision tail")?;
        let repeat = read_u32(payload, p4 as usize + 8).ok_or("truncated collision tail")?;
        let tail_zero = read_u32(payload, p4 as usize + 12).ok_or("truncated collision tail")?;
        // The stored anchor references the ABSOLUTE pv (author compares the
        // raw unpacked `pv`, not the base-relative index).
        if anchor != base + pv - 32 || repeat != nt || tail_zero != 0 {
            return Err("invalid collision tail header".into());
        }
        if p4 + 48 + nodes * 32 != payload.len() as u32 {
            return Err("unknown collision trailing data".into());
        }
        if payload[0..32] != payload[p4 as usize + 16..p4 as usize + 48] {
            return Err("collision bounds copies disagree".into());
        }
        let mut vertices = Vec::with_capacity(nv as usize);
        for i in 0..nv {
            let at = pv as usize + i as usize * 16;
            let mut vertex = [0f32; 3];
            for axis in 0..3 {
                vertex[axis] =
                    read_f32(payload, at + axis * FLAT).ok_or("truncated collision vertex")?;
            }
            vertices.push(vertex);
        }
        let mut triangles = Vec::with_capacity(nt as usize);
        for i in 0..nt {
            let at = pt as usize + i as usize * 16;
            let a = read_u32(payload, at).ok_or("truncated collision triangle")?;
            let b = read_u32(payload, at + 4).ok_or("truncated collision triangle")?;
            let c = read_u32(payload, at + 8).ok_or("truncated collision triangle")?;
            if a >= nv || b >= nv || c >= nv {
                return Err("collision triangle index out of range".into());
            }
            triangles.push([a, b, c]);
        }
        for vertex in &vertices {
            for axis in 0..3 {
                if vertex[axis] < low[axis] - 1e-4 || vertex[axis] > high[axis] + 1e-4 {
                    return Err("collision vertices outside bounds".into());
                }
            }
        }
        let mut tree = Vec::with_capacity(nodes as usize);
        for i in 0..nodes {
            let at = p4 as usize + 48 + i as usize * 32;
            let mut node = [0i32; 6];
            for (index, slot) in node.iter_mut().enumerate() {
                *slot = read_i32(payload, at + index * FLAT).ok_or("truncated collision tree")?;
            }
            let split_lo = read_f32(payload, at + 24).ok_or("truncated collision tree")?;
            let split_hi = read_f32(payload, at + 28).ok_or("truncated collision tree")?;
            tree.push((
                node[0] as u32,
                node[1] as u32,
                node[2],
                node[3],
                node[4],
                node[5],
                split_lo,
                split_hi,
            ));
        }
        Ok(Self {
            payload: payload.to_vec(),
            low,
            high,
            magic,
            depth,
            triangles,
            vertices,
            tree,
            pt,
            pv,
            p4,
            p3,
        })
    }

    /// Validate the observed branch schema against triangle coordinate extrema.
    pub fn validate_tree(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        let mut covered = Vec::new();
        if self.tree.is_empty() {
            return Err("no explicit collision tree".into());
        }
        let mut stack = vec![0usize];
        while let Some(index) = stack.pop() {
            if index >= self.tree.len() || !seen.insert(index) {
                return Err("invalid collision tree reference/cycle".into());
            }
            let (_, axis, nl, il, nr, ir, split_lo, split_hi) = self.tree[index];
            if axis > 2 {
                return Err("unsupported collision branch encoding".into());
            }
            let mut child = |count: i32, start: i32| -> Result<Vec<usize>, String> {
                if count == -1 {
                    if start < 0 || start as usize >= self.tree.len() {
                        return Err("invalid collision tree reference".into());
                    }
                    stack.push(start as usize);
                    return Ok(Vec::new());
                }
                if count <= 0
                    || start < 0
                    || start
                        .checked_add(count)
                        .is_none_or(|end| end < 0 || end as usize > self.triangles.len())
                {
                    return Err("invalid leaf range".into());
                }
                Ok((start..start + count).map(|i| i as usize).collect())
            };
            let left = child(nl, il)?;
            let right = child(nr, ir)?;
            let maximum = extreme(
                &self.triangles,
                &self.vertices,
                &left,
                axis as usize,
                Extreme::Max,
            );
            let minimum = extreme(
                &self.triangles,
                &self.vertices,
                &right,
                axis as usize,
                Extreme::Min,
            );
            if let Some(maximum) = maximum {
                if maximum > split_lo + 1e-4 {
                    return Err("tree split excludes collision geometry".into());
                }
            }
            if let Some(minimum) = minimum {
                if minimum < split_hi - 1e-4 {
                    return Err("tree split excludes collision geometry".into());
                }
            }
            covered.extend(left);
            covered.extend(right);
        }
        if seen.len() != self.tree.len() {
            return Err("collision tree does not cover each triangle exactly once".into());
        }
        let mut sorted = covered;
        sorted.sort_unstable();
        if sorted.len() != self.triangles.len()
            || sorted
                .iter()
                .enumerate()
                .any(|(index, value)| *value as usize != index)
        {
            return Err("collision tree does not cover each triangle exactly once".into());
        }
        Ok(())
    }

    /// Confirm a verified native six-face box: 12 triangles / 24 vertices,
    /// exactly the 8 box corners, six rectangle faces of two triangles each.
    pub fn validate_box(&self) -> Result<(), String> {
        self.validate_tree()?;
        if self.triangles.len() != BOX_TRIANGLES as usize
            || self.vertices.len() != BOX_VERTICES as usize
        {
            return Err("expected native six-face box".into());
        }
        let corners: std::collections::HashSet<[u32; 3]> = self
            .vertices
            .iter()
            .map(|v| [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()])
            .collect();
        let mut expected = std::collections::HashSet::new();
        for &x in &[self.low[0], self.high[0]] {
            for &y in &[self.low[1], self.high[1]] {
                for &z in &[self.low[2], self.high[2]] {
                    expected.insert([x.to_bits(), y.to_bits(), z.to_bits()]);
                }
            }
        }
        if corners != expected || corners.len() != 8 {
            return Err("proxy is not a box".into());
        }
        let mut faces: std::collections::HashMap<(usize, u32), u32> =
            std::collections::HashMap::new();
        for tri in &self.triangles {
            let mut fixed = None;
            for axis in 0..3 {
                let a = self.vertices[tri[0] as usize][axis];
                let b = self.vertices[tri[1] as usize][axis];
                let c = self.vertices[tri[2] as usize][axis];
                if a.to_bits() == b.to_bits() && b.to_bits() == c.to_bits() {
                    if fixed.is_some() {
                        return Err("invalid box face triangle".into());
                    }
                    fixed = Some((axis, a.to_bits()));
                }
            }
            let Some(key) = fixed else {
                return Err("invalid box face triangle".into());
            };
            *faces.entry(key).or_insert(0) += 1;
        }
        if faces.len() != 6 || faces.values().any(|count| *count != 2) {
            return Err("incomplete box surface".into());
        }
        Ok(())
    }

    /// Positive affine transform into `low..high` at destination `destination`
    /// (the collision section's absolute position in the target model).
    pub fn transformed(
        &self,
        low: [f32; 3],
        high: [f32; 3],
        destination: u32,
    ) -> Result<Vec<u8>, String> {
        self.validate_box()?;
        if destination % 16 != 0 {
            return Err("collision destination must be 16-byte aligned".into());
        }
        for axis in 0..3 {
            if !low[axis].is_finite() || !high[axis].is_finite() || high[axis] <= low[axis] {
                return Err("invalid proxy dimensions".into());
            }
        }
        let scale: [f32; 3] = [
            (high[0] - low[0]) / (self.high[0] - self.low[0]),
            (high[1] - low[1]) / (self.high[1] - self.low[1]),
            (high[2] - low[2]) / (self.high[2] - self.low[2]),
        ];
        let transform = |x: f32, axis: usize| (x - self.low[axis]) * scale[axis] + low[axis];
        let write_u32 = |payload: &mut [u8], at: usize, value: u32| {
            payload[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        let write_f32 = |payload: &mut [u8], at: usize, value: f32| {
            payload[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        let mut payload = self.payload.clone();
        // Bounds copies: header (offset 0, xyz + pad each) and tail mirror.
        let mut bounds = Vec::with_capacity(32);
        for value in low {
            bounds.extend_from_slice(&value.to_le_bytes());
        }
        bounds.extend_from_slice(&[0; 4]);
        for value in high {
            bounds.extend_from_slice(&value.to_le_bytes());
        }
        bounds.extend_from_slice(&[0; 4]);
        payload[0..32].copy_from_slice(&bounds);
        let mirror = self.p4 as usize + 16;
        payload[mirror..mirror + 32].copy_from_slice(&bounds);
        // Header words 32..68: magic, depth, nt, zero, nv, pt, pv, p4, p3.
        write_u32(&mut payload, 32, self.magic);
        write_u32(&mut payload, 36, self.depth);
        write_u32(&mut payload, 40, self.triangles.len() as u32);
        write_u32(&mut payload, 44, 0);
        write_u32(&mut payload, 48, self.vertices.len() as u32);
        write_u32(&mut payload, 52, destination + self.pt);
        write_u32(&mut payload, 56, destination + self.pv);
        write_u32(&mut payload, 60, destination + self.p4);
        write_u32(&mut payload, 64, destination + self.p3);
        // Transformed vertices (stride 16, xyz + pad).
        for (index, vertex) in self.vertices.iter().enumerate() {
            let at = self.pv as usize + index * 16;
            for axis in 0..3 {
                write_f32(
                    &mut payload,
                    at + axis * FLAT,
                    transform(vertex[axis], axis),
                );
            }
        }
        // Triangle indices unchanged; flag word (offset +12) stays zero.
        for (index, tri) in self.triangles.iter().enumerate() {
            let at = self.pt as usize + index * 16;
            write_u32(&mut payload, at, tri[0]);
            write_u32(&mut payload, at + 4, tri[1]);
            write_u32(&mut payload, at + 8, tri[2]);
        }
        // Tail header + transformed tree splits.
        write_u32(&mut payload, self.p4 as usize, destination + self.pv - 32);
        write_u32(&mut payload, self.p4 as usize + 4, self.tree.len() as u32);
        write_u32(
            &mut payload,
            self.p4 as usize + 8,
            self.triangles.len() as u32,
        );
        write_u32(&mut payload, self.p4 as usize + 12, 0);
        for (index, node) in self.tree.iter().enumerate() {
            let (_, axis, nl, il, nr, ir, split_lo, split_hi) = *node;
            let at = self.p4 as usize + 48 + index * 32;
            write_u32(&mut payload, at, 0); // tag
            write_u32(&mut payload, at + 4, axis);
            for (offset, value) in [(8usize, nl), (12, il), (16, nr), (20, ir)] {
                payload[at + offset..at + offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            write_f32(&mut payload, at + 24, transform(split_lo, axis as usize));
            write_f32(&mut payload, at + 28, transform(split_hi, axis as usize));
        }
        let check = Collision::parse(&payload, destination)?;
        check.validate_tree()?;
        Ok(payload)
    }

    fn payload_len(&self) -> usize {
        (self.p4 + 48 + self.tree.len() as u32 * 32) as usize
    }
}

/// Replace the single `0x80003` collision section of `model` with the source
/// box rescaled to `low..high` (author `replace_box_collision`).
pub fn replace_box_collision(
    model: &[u8],
    source_bytes: &[u8],
    low: [f32; 3],
    high: [f32; 3],
) -> Result<Vec<u8>, String> {
    let model_file = Rw4File::parse(model).map_err(|e| format!("model RW4 parse: {e}"))?;
    let source_file =
        Rw4File::parse(source_bytes).map_err(|e| format!("collision source parse: {e}"))?;
    let mut dst = model_file.sections_of_type(SectionType::COLLISION_MESH);
    let dst_number = match (dst.next(), dst.next()) {
        (Some(section), None) => section.number,
        _ => return Err("expected one collision section".into()),
    };
    let mut src = source_file.sections_of_type(SectionType::COLLISION_MESH);
    let src_payload = match (src.next(), src.next()) {
        (Some(section), None) => source_file
            .payload(source_bytes, section.number)
            .map_err(|e| e.to_string())?,
        _ => return Err("expected one collision section in source".into()),
    };
    // The source RW4 is a standalone container; parse it with its own section
    // base, then transplant against the target's destination position.
    let proxy = Collision::parse(
        src_payload,
        src_pos(&source_file, SectionType::COLLISION_MESH),
    )?;
    // First pass with a placeholder of the final size fixes the section
    // layout; the transformed payload keeps the source length, so the second
    // pass lands at the same position and the embedded references match it.
    let placeholder = vec![0u8; proxy.payload_len()];
    let mut replacements = std::collections::HashMap::new();
    replacements.insert(dst_number, placeholder);
    let pass1 = rw4::writer::replace_template_sections(model, &replacements)?;
    let pass1_file = Rw4File::parse(&pass1).map_err(|e| format!("collision pass1 parse: {e}"))?;
    let destination = pass1_file
        .section(dst_number)
        .ok_or("collision section lost")?
        .pos;
    let payload = proxy.transformed(low, high, destination)?;
    replacements.insert(dst_number, payload);
    let rebuilt = rw4::writer::replace_template_sections(model, &replacements)?;
    let check_file =
        Rw4File::parse(&rebuilt).map_err(|e| format!("collision rebuild parse: {e}"))?;
    let section = check_file
        .section(dst_number)
        .ok_or("collision section lost")?;
    let final_payload = check_file
        .payload(&rebuilt, dst_number)
        .map_err(|e| e.to_string())?;
    let check = Collision::parse(final_payload, section.pos)?;
    check.validate_tree()?;
    Ok(rebuilt)
}

fn src_pos(file: &Rw4File, type_code: u32) -> u32 {
    file.sections_of_type(type_code)
        .next()
        .map(|section| section.pos)
        .unwrap_or(0)
}

enum Extreme {
    Max,
    Min,
}

/// Coordinate extreme of `triangles`' vertices along `axis`
/// (author `max(...)`/`min(...)` generator expressions).
fn extreme(
    triangles: &[[u32; 3]],
    vertices: &[[f32; 3]],
    indices: &[usize],
    axis: usize,
    which: Extreme,
) -> Option<f32> {
    indices
        .iter()
        .flat_map(|t| triangles[*t])
        .map(|v| vertices[v as usize][axis])
        .fold(None, |acc: Option<f32>, value| {
            Some(match acc {
                None => value,
                Some(current) => match which {
                    Extreme::Max => current.max(value),
                    Extreme::Min => current.min(value),
                },
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &[u8] = include_bytes!("../../resources/sc5/collision-source.rw4");

    fn source_payload() -> (Vec<u8>, u32) {
        let file = Rw4File::parse(SOURCE).expect("collision source parses");
        let section = file
            .sections_of_type(SectionType::COLLISION_MESH)
            .next()
            .expect("collision section")
            .number;
        let payload = file.payload(SOURCE, section).expect("payload").to_vec();
        let pos = file.section(section).expect("section").pos;
        (payload, pos)
    }

    #[test]
    fn source_is_a_verified_box() {
        let (payload, pos) = source_payload();
        let collision = Collision::parse(&payload, pos).expect("parse");
        collision.validate_box().expect("box proxy");
        assert_eq!(collision.triangles.len(), 12);
        assert_eq!(collision.vertices.len(), 24);
    }

    #[test]
    fn transform_rescales_and_roundtrips() {
        let (payload, pos) = source_payload();
        let proxy = Collision::parse(&payload, pos).expect("parse");
        let low = [-12.0, -10.0, 0.0];
        let high = [12.0, 10.0, 44.0];
        // Destination must be 16-aligned; use the source section position.
        let destination = (pos + 15) & !15;
        let transformed = proxy
            .transformed(low, high, destination)
            .expect("transform");
        let check = Collision::parse(&transformed, destination).expect("re-parse");
        check.validate_tree().expect("tree valid");
        for axis in 0..3 {
            assert!((check.low[axis] - low[axis]).abs() < 1e-4);
            assert!((check.high[axis] - high[axis]).abs() < 1e-4);
        }
        assert_eq!(check.triangles, proxy.triangles);
        assert_eq!(check.tree.len(), proxy.tree.len());
    }

    #[test]
    fn replace_box_collision_swaps_the_model_section() {
        let template = include_bytes!("../../resources/sc5/175EFB4E.research-template.rw4");
        let rebuilt = replace_box_collision(template, SOURCE, [-6.0, -8.0, 0.0], [6.0, 8.0, 24.0])
            .expect("replace");
        let file = Rw4File::parse(&rebuilt).expect("rebuilt parses");
        let section = file
            .sections_of_type(SectionType::COLLISION_MESH)
            .next()
            .expect("collision section");
        let payload = file.payload(&rebuilt, section.number).expect("payload");
        let check = Collision::parse(payload, section.pos).expect("parse");
        assert!((check.low[2] - 0.0).abs() < 1e-4);
        assert!((check.high[2] - 24.0).abs() < 1e-4);
        assert!((check.low[0] + 6.0).abs() < 1e-4);
    }
}
