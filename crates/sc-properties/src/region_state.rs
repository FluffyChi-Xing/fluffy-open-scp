//! Read-only map/path subset of retail EcoGame states. Source: cEcoGame::Read,
//! cEcoMap::Read, cEcoUnit::Read, cEcoPathSet::Read and SplineRepForSegment.
//! Retail v17/v20 headers and v4 map sums differ from the older dev source.
use serde::Serialize;
use std::collections::HashSet;
use std::io::Read;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSource {
    pub site: String,
    pub origin: [f32; 2],
    pub bounds: Option<[f32; 4]>,
    pub maps: Vec<StateMap>,
    pub curves: Vec<StateCurve>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLayers {
    pub sources: Vec<StateSource>,
    pub regional_units: Vec<StateUnit>,
}

fn state_file(directory: &std::path::Path) -> Result<Option<RegionState>, String> {
    if !directory.is_dir() {
        return Ok(None);
    }
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "egb"))
        .collect();
    files.sort_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());
    let Some(file) = files.last() else {
        return Ok(None);
    };
    let input = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(input)
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("state exceeds decompression limit".into());
    }
    read_state(&bytes)
        .map(Some)
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// Discover only the standard Games/account/region hierarchy. Match the state
/// environment before loading city resource fields; never borrow another map.
pub fn discover_save(root: &std::path::Path, group: u32, plots: &[crate::region_3d::Region3DPlot]) -> Option<(std::path::PathBuf, SaveLayers)> {
    let mut candidates = Vec::new();
    for account in std::fs::read_dir(root).ok()?.flatten() {
        let Ok(regions)=std::fs::read_dir(account.path()) else {continue};
        for region in regions.flatten() {
            let path=region.path();
            let zero=path.join("0");
            if !zero.is_dir() {continue;}
            let modified=std::fs::read_dir(&zero).ok().into_iter().flatten().flatten()
                .filter(|e|e.path().extension().is_some_and(|x|x=="egb"))
                .filter_map(|e|e.metadata().ok()?.modified().ok()).max();
            candidates.push((modified,path));
        }
    }
    candidates.sort_by(|a,b|b.0.cmp(&a.0).then_with(||a.1.cmp(&b.1)));
    for (_,path) in candidates {
        if let Ok(Some(state))=state_file(&path.join("0")) {
            if state.environment==group {
                if let Ok(layers)=load_save(&path,group,plots) {return Some((path,layers));}
            }
        }
    }
    None
}

/// Region zero is the authoritative identity/outer network. City maps cover only
/// their 2048m simulation squares; unavailable cells are never filled by inference.
pub fn load_save(
    directory: &std::path::Path,
    group: u32,
    plots: &[crate::region_3d::Region3DPlot],
) -> Result<SaveLayers, String> {
    let region = state_file(&directory.join("0"))?.ok_or("save has no region state")?;
    if region.environment != group {
        return Err(format!(
            "save region {:08X} does not match {group:08X}",
            region.environment
        ));
    }
    let mut sources = vec![StateSource {
        site: "0".into(),
        origin: [0.0; 2],
        bounds: None,
        maps: vec![],
        curves: region.curves,
    }];
    for plot in plots.iter().filter(|p| p.kind == "city") {
        let Some(city) = state_file(&directory.join(&plot.uid))? else {
            continue;
        };
        if city.maps.iter().any(|m| m.size != 128) {
            return Err("unsupported city map dimensions".into());
        }
        sources.push(StateSource {
            site: plot.uid.clone(),
            origin: [plot.x, plot.y],
            bounds: Some([
                plot.x - 1024.0,
                plot.y - 1024.0,
                plot.x + 1024.0,
                plot.y + 1024.0,
            ]),
            // City saves supply reference resource fields only. Never replace the
            // empty-map network, height or ecology with played-city state.
            maps: city
                .maps
                .into_iter()
                .filter(|m| {
                    matches!(
                        m.id,
                        0xD779C976 | 0xD74AF22B | 0x76D10FF2 | 0x0F92DC56 | 0x5B9DB0F3
                    )
                })
                .collect(),
            curves: vec![],
        });
    }
    Ok(SaveLayers {
        sources,
        regional_units: region.units,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateMap {
    pub id: u32,
    pub size: usize,
    pub values: Vec<i32>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateCurve {
    pub entry_id: u32,
    pub path: usize,
    pub controls: [[f32; 3]; 4],
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionState {
    pub version: u32,
    pub environment: u32,
    pub maps: Vec<StateMap>,
    pub curves: Vec<StateCurve>,
    pub units: Vec<StateUnit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateUnit {
    pub id: u32,
    pub graphics_group: u32,
    pub graphics_state: u32,
    pub scale: f32,
    pub rotation: [f32; 9],
    pub position: [f32; 3],
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.pos.checked_add(n).ok_or("state length overflow")?;
        let out = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| format!("truncated state at {}", self.pos))?;
        self.pos = end;
        Ok(out)
    }
    fn u(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn count(&mut self, limit: usize) -> Result<usize, String> {
        let n = self.u()? as usize;
        if n > limit {
            return Err(format!("state count {n} exceeds {limit}"));
        }
        Ok(n)
    }
    fn vec3(&mut self) -> Result<[f32; 3], String> {
        let v = [
            f32::from_bits(self.u()?),
            f32::from_bits(self.u()?),
            f32::from_bits(self.u()?),
        ];
        if v.iter().any(|v| !v.is_finite()) {
            return Err("non-finite path coordinate".into());
        }
        Ok(v)
    }
    fn slots(&mut self) -> Result<(usize, HashSet<usize>), String> {
        let count = self.count(1_000_000)?;
        let n = self.count(count)?;
        let mut free = HashSet::new();
        for _ in 0..n {
            let slot = self.u()? as usize;
            if slot >= count || !free.insert(slot) {
                return Err("invalid free slot".into());
            }
        }
        Ok((count, free))
    }
}

/// Input must already be decompressed. Never searches arbitrary bytes for paths.
pub fn read_state(bytes: &[u8]) -> Result<RegionState, String> {
    let mut r = Reader { bytes, pos: 0 };
    let version = r.u()?;
    if !matches!(version, 17 | 20) {
        return Err(format!("unsupported EcoGame version {version}"));
    }
    r.take(8)?; // game ID and scripts version
    let addons = r.count(256)?;
    r.take(addons * 8)?;
    let environment = r.u()?;
    let skills = r.count(16384)?;
    r.take(skills * 8)?;
    r.take(8)?; // ticks, next UID
    if version == 17 {
        // Older retail files retain an additional empty skill-state collection.
        if r.u()? != 0 {
            return Err("unsupported nonempty v17 skill-state extension".into());
        }
    }
    let n = r.count(1024)?;
    let map_version = r.u()?;
    if !matches!(map_version, 3 | 4) {
        return Err(format!("unsupported map version {map_version}"));
    }
    let mut maps = Vec::new();
    for _ in 0..n {
        let id = r.u()?;
        let count = r.count(4_194_304)?;
        let size = (count as f64).sqrt() as usize;
        if size == 0 || size * size != count {
            return Err("non-square state map".into());
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(r.u()? as i32);
        }
        let sum = if map_version == 4 {
            ((r.u()? as u64) << 32 | r.u()? as u64) as i64
        } else {
            r.u()? as i32 as i64
        };
        if values.iter().map(|&v| i64::from(v)).sum::<i64>() != sum {
            return Err(format!("map {id:08X} sum mismatch"));
        }
        maps.push(StateMap { id, size, values });
    }
    let units = r.count(1_000_000)?;
    let max_slots = r.count(1_000_000)?;
    if units > max_slots || r.u()? != 13 {
        return Err("unsupported unit table".into());
    }
    let mut used = HashSet::new();
    let mut unit_records = Vec::new();
    for _ in 0..units {
        let slot = r.u()? as usize;
        if slot >= max_slots || !used.insert(slot) {
            return Err("invalid unit slot".into());
        }
        let id = r.u()?;
        r.u()?; // UID
        let graphics_group = r.u()?;
        let graphics_state = r.u()?;
        r.take(8)?; // map XY
        let flags = u16::from_be_bytes(r.take(2)?.try_into().unwrap());
        if flags & !15 != 0 {
            return Err("unsupported transform flags".into());
        }
        let scale = if flags & 1 != 0 {
            f32::from_bits(r.u()?)
        } else {
            1.0
        };
        let mut rotation = [1., 0., 0., 0., 1., 0., 0., 0., 1.];
        if flags & 2 != 0 {
            for axis in rotation.chunks_mut(3) {
                axis.copy_from_slice(&r.vec3()?);
            }
        }
        let position = if flags & 4 != 0 { r.vec3()? } else { [0.; 3] };
        if !scale.is_finite() {
            return Err("non-finite unit scale".into());
        }
        unit_records.push(StateUnit {
            id,
            graphics_group,
            graphics_state,
            scale,
            rotation,
            position,
        });
        r.take(16)?; // control flags, zone, creation tick
        let bins = r.count(16383)?;
        r.take(bins * 8)?;
        let connected = r.count(max_slots)?;
        r.take(connected * 4)?;
    }
    if r.u()? != 6 {
        return Err("unsupported path-set version".into());
    }
    let (n, free_points) = r.slots()?;
    let mut points = Vec::new();
    for _ in 0..n {
        points.push(r.vec3()?);
    }
    let (n, free_tangents) = r.slots()?;
    let mut tangents = Vec::new();
    for _ in 0..n {
        tangents.push(r.vec3()?);
    }
    let (n, free) = r.slots()?;
    let mut segments = Vec::new();
    for i in 0..n {
        if free.contains(&i) {
            continue;
        }
        let mut s = [0i32; 7];
        for v in &mut s {
            *v = r.u()? as i32;
        }
        segments.push(s);
    }
    let point_data = r.count(1_000_000)?;
    if point_data != points.len() {
        return Err("path point metadata count mismatch".into());
    }
    r.take(point_data * 16)?; // first segment/dir and three point-data words
    let (n, free) = r.slots()?;
    let mut paths = vec![None; n];
    for (i, path) in paths.iter_mut().enumerate() {
        if free.contains(&i) {
            continue;
        }
        let id = r.u()?;
        r.take(20)?;
        *path = Some(id);
    }
    let (n, free) = r.slots()?;
    r.take((n - free.len()) * 24)?; // path groups
    let mut curves = Vec::new();
    for s in segments {
        let point = |i: i32| -> Result<[f32; 3], String> {
            if i < 0 || free_points.contains(&(i as usize)) {
                return Err("dead path point".into());
            }
            points
                .get(i as usize)
                .copied()
                .ok_or_else(|| "invalid path point".into())
        };
        let a = point(s[0])?;
        let b = point(s[1])?;
        let tangent = |i: i32| -> Result<[f32; 3], String> {
            if i < 0 {
                return Ok(std::array::from_fn(|k| b[k] - a[k]));
            }
            if free_tangents.contains(&(i as usize)) {
                return Err("dead tangent".into());
            }
            tangents
                .get(i as usize)
                .copied()
                .ok_or_else(|| "invalid tangent".into())
        };
        let ta = tangent(s[2])?;
        let tb = tangent(s[3])?;
        let entry_id = paths
            .get(s[6] as usize)
            .copied()
            .flatten()
            .ok_or("invalid segment path")?;
        curves.push(StateCurve {
            entry_id,
            path: s[6] as usize,
            controls: [
                a,
                std::array::from_fn(|k| a[k] + ta[k] / 3.0),
                std::array::from_fn(|k| b[k] - tb[k] / 3.0),
                b,
            ],
        });
    }
    Ok(RegionState {
        version,
        environment,
        maps,
        curves,
        units: unit_records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_and_unknown_states() {
        assert!(read_state(&[]).is_err());
        assert!(read_state(&20u32.to_be_bytes()).is_err());
        assert!(
            read_state(&99u32.to_be_bytes())
                .unwrap_err()
                .contains("unsupported")
        );
    }
    fn sample_state(version: u32) -> Vec<u8> {
        let mut words = vec![version, 1, 1, 0, 0xC04182E4, 0, 0, 1];
        if version == 17 {
            words.push(0);
        }
        words.extend([1, 4, 0xD779C976, 1, 7, 0, 7, 0, 0, 13]);
        words.extend([6, 2, 0]); // path version and two live points
        words.extend([0f32, 0., 10., 12., 0., 10.].map(f32::to_bits));
        words.extend([2, 0]);
        words.extend([12f32, 0., 0., 12., 0., 0.].map(f32::to_bits));
        words.extend([1, 0, 0, 1, 0, 1, 0, 0, 0]); // one segment
        words.extend([2, 0, 0, 0, 0, 0, 0, 0, 0]); // point link/data arrays
        words.extend([1, 0, 0x017A430A, 0, 0, 1, u32::MAX, u32::MAX]);
        words.extend([0, 0]); // no groups
        words.into_iter().flat_map(u32::to_be_bytes).collect()
    }
    #[test]
    fn reads_retail_header_variants_and_exact_bezier_controls() {
        for version in [17, 20] {
            let bytes = sample_state(version);
            let state = read_state(&bytes).unwrap();
            assert_eq!(state.maps[0].values, vec![7]);
            assert_eq!(state.curves[0].entry_id, 0x017A430A);
            assert_eq!(
                state.curves[0].controls,
                [[0., 0., 10.], [4., 0., 10.], [8., 0., 10.], [12., 0., 10.]]
            );
            for end in [8, 32, bytes.len() - 1] {
                assert!(read_state(&bytes[..end]).is_err());
            }
        }
    }
    #[test]
    fn rejects_corrupt_field_sum_and_invalid_slots() {
        let mut bytes = sample_state(20);
        bytes[48..52].copy_from_slice(&9u32.to_be_bytes());
        assert!(read_state(&bytes).unwrap_err().contains("sum mismatch"));
        let mut r = Reader {
            bytes: &[0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
            pos: 0,
        };
        assert!(r.slots().is_err());
    }
}
