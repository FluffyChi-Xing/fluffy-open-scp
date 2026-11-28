//! Generate an original, standalone RW4 model for cross-reader verification.

use rw4::{Rw4File, SectionType, build_file, writer_section};
use std::{env, fs, io};

const STRIDE: u16 = 64;
const ELEMENTS: [(u16, u16, u16, u8); 6] = [
    (0, 2, 0, 0),
    (12, 2, 3, 0),
    (24, 4, 10, 0),
    (28, 1, 5, 0),
    (36, 3, 5, 1),
    (52, 2, 6, 0),
];
const TRIANGLE: [[f32; 3]; 3] = [[0., 0., 0.], [2., 0., 0.], [0., 1., 0.]];
const QUAD: [[f32; 3]; 4] = [[10., 0., 1.], [12., 0., 1.], [12., 2., 1.], [10., 2., 1.]];
const TRIANGLE_INDICES: [u16; 3] = [0, 1, 2];
const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];
const TEXTURE_BASES: [u32; 2] = [0x1000_0100, 0x2000_0200];

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn vertex_format() -> Vec<u8> {
    let mut payload = vec![0; 12];
    payload.extend((ELEMENTS.len() as u16).to_le_bytes());
    payload.extend(STRIDE.to_be_bytes());
    payload.extend([0; 8]);
    for (offset, ty, usage, index) in ELEMENTS {
        payload.push(0);
        payload.extend(offset.to_be_bytes());
        payload.extend(ty.to_be_bytes());
        payload.extend(usage.to_be_bytes());
        payload.extend([index, 0, 0, 0, 0]);
    }
    payload
}

fn vertices(positions: &[[f32; 3]], tint_column: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    for (index, position) in positions.iter().enumerate() {
        let [x, y, _] = *position;
        for value in position.iter().copied().chain([0., 0., 1.]) {
            payload.extend(value.to_le_bytes());
        }
        payload.extend([255, 0, tint_column, index as u8]);
        for value in [x / 2., y / 2., x, y, x / 4., y / 4., 1., 0., 0.] {
            payload.extend(value.to_le_bytes());
        }
    }
    payload
}

fn material(format: &[u8], texture_base: u32) -> Vec<u8> {
    let mut payload = words(&[(32 + format.len() + 24 + 7 * 24) as u32]);
    payload.extend([0; 28]);
    payload.extend(format);
    payload.extend([0; 24]);
    payload.extend(words(&[0x2D, 0, 0, 0, 0, 0]));
    for slot in 0..6 {
        payload.extend(words(&[slot, 0, texture_base + slot, 0, 0, 0]));
    }
    payload
}

fn fixture() -> Vec<u8> {
    let format = vertex_format();
    let mut sections = vec![writer_section(SectionType::VERTEX_FORMAT, format.clone())];
    for (positions, indices, tint_column) in [
        (&TRIANGLE[..], &TRIANGLE_INDICES[..], 3),
        (&QUAD[..], &QUAD_INDICES[..], 7),
    ] {
        let va = sections.len() as u32;
        let vertex_count = positions.len() as u32;
        let index_count = indices.len() as u32;
        sections.extend([
            writer_section(
                SectionType::VERTEX_ARRAY,
                words(&[0, 0, 0, vertex_count, 8, STRIDE.into(), va + 1]),
            ),
            writer_section(SectionType::BLOB, vertices(positions, tint_column)),
            writer_section(
                SectionType::BLOB,
                indices
                    .iter()
                    .flat_map(|index| index.to_le_bytes())
                    .collect(),
            ),
            writer_section(
                SectionType::TRIANGLE_ARRAY,
                words(&[0, 0, index_count, 8, 101, 4, va + 2]),
            ),
            writer_section(
                SectionType::MESH,
                words(&[
                    40,
                    4,
                    va + 3,
                    index_count / 3,
                    1,
                    0,
                    index_count,
                    0,
                    vertex_count,
                    va,
                ]),
            ),
        ]);
    }
    sections.extend(
        TEXTURE_BASES.map(|base| writer_section(SectionType::MATERIAL, material(&format, base))),
    );
    sections.extend([
        writer_section(SectionType::MESH_MATERIAL_ASSIGNMENT, words(&[5, 1, 11])),
        writer_section(SectionType::MESH_MATERIAL_ASSIGNMENT, words(&[10, 1, 12])),
    ]);
    build_file(1, &sections, &[])
}

fn verify(bytes: &[u8]) -> rw4::Result<()> {
    let file = Rw4File::parse(bytes)?;
    assert_eq!(file.sections().len(), 15);
    let bindings = file.decode_mesh_material_bindings(bytes);
    assert_eq!(bindings.len(), 2);
    for ((mesh_number, material_number, positions, triangles), binding) in [
        (5, 11, &TRIANGLE[..], &[[0, 1, 2]][..]),
        (10, 12, &QUAD[..], &[[0, 1, 2], [0, 2, 3]][..]),
    ]
    .into_iter()
    .zip(bindings)
    {
        assert_eq!(
            (binding.mesh_section, binding.material_section),
            (mesh_number, material_number)
        );
        let mesh = file.decode_mesh(bytes, mesh_number)?;
        assert_eq!(mesh.triangles, triangles);
        assert_eq!(
            mesh.vertices
                .iter()
                .map(|vertex| vertex.position().unwrap())
                .collect::<Vec<_>>(),
            positions
        );
        let material = file.decode_material(bytes, material_number)?;
        let texture_base = TEXTURE_BASES[(material_number - 11) as usize];
        assert_eq!(material.texture_refs().len(), 7);
        for slot in 0..6 {
            assert!(
                material
                    .texture_refs()
                    .iter()
                    .any(|reference| reference.slot == slot
                        && reference.texture_instance == texture_base + slot)
            );
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os().nth(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: two_mesh_material_fixture <output.rw4>",
        )
    })?;
    let bytes = fixture();
    verify(&bytes)?;
    fs::write(&path, bytes)?;
    verify(&fs::read(path)?)?;
    Ok(())
}
