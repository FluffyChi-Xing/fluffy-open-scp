use rw4::{FileType, Rw4File, SectionType, TEXTURE_TYPE_RAW_BGRA, writer};

const ERROR: &str = "road_mesh_raster_invalid";
// Native SimCity_Game model 5D801461, raster 11 and blob 10:
// two white BGRA mips, 2x2 followed by 1x1.
const PIXELS: [u8; 20] = [0xFF; 20];

pub(super) fn complete(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let model = Rw4File::parse(bytes).map_err(|e| format!("{ERROR}: {e}"))?;
    let mut rasters = model
        .sections()
        .iter()
        .filter(|s| s.type_code == SectionType::TEXTURE);
    let raster = rasters.next().ok_or(ERROR)?;
    if model.file_type() != FileType::Model || rasters.next().is_some() {
        return Err(ERROR.into());
    }
    let descriptor = model
        .payload(bytes, raster.number)
        .map_err(|e| format!("{ERROR}: {e}"))?;
    let descriptor: &[u8; 32] = descriptor.try_into().map_err(|_| ERROR)?;
    let word = |offset| u32::from_le_bytes(descriptor[offset..offset + 4].try_into().unwrap());
    if word(0) != TEXTURE_TYPE_RAW_BGRA || word(12) != 0x0002_0002 || word(16) >> 8 != 2 {
        return Err(ERROR.into());
    }
    let blob = word(28);
    let completed = if blob as usize == model.sections().len() {
        // The merged model writer declares this raster's next section but does
        // not emit its pixel data. Preserve the descriptor and add that section.
        let mut sections = Vec::with_capacity(model.sections().len() + 1);
        for section in model.sections() {
            if !section.fixups.is_empty() {
                return Err(ERROR.into());
            }
            sections.push(writer::SectionSpec {
                type_code: section.type_code,
                alignment: section.alignment,
                payload: model
                    .payload(bytes, section.number)
                    .map_err(|e| format!("{ERROR}: {e}"))?
                    .to_vec(),
                fixups: Vec::new(),
            });
        }
        sections.push(writer::section(SectionType::BLOB, PIXELS.to_vec()));
        let mut completed = writer::build(1, &sections, &[]);
        completed[0x64..0x68].copy_from_slice(&bytes[0x64..0x68]);
        completed
    } else {
        bytes.to_vec()
    };
    let model = Rw4File::parse(&completed).map_err(|e| format!("{ERROR}: {e}"))?;
    let texture = model
        .decode_texture(&completed, raster.number)
        .map_err(|e| format!("{ERROR}: {e}"))?;
    if texture.blob.len() != PIXELS.len() {
        return Err(ERROR.into());
    }
    Ok(completed)
}
