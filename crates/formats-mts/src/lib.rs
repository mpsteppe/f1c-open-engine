//! Reader for the CUBE_MTS_4.10 model format used by F1 Challenge '99-'02.
//!
//! See `specs/MTS_FORMAT.md` for the format description. This crate parses
//! model bytes owned by the caller; it never opens files itself.

use std::fmt;

/// 16-byte magic that opens a 4.10 file (13 chars + 3 NUL).
const MAGIC_410: [u8; 16] = *b"CUBE_MTS_4.10\0\0\0";
/// 14-byte magic that opens the geometry block (13 chars + NUL). Spec v1.1:
/// bytes 14-15 vary, so they must never be checked.
const GEOMETRY_MAGIC: &[u8] = b"CUBE_MTS_4.10\0";
/// Geometry magic of the unsupported legacy variant.
const MAGIC_401: &[u8] = b"CUBE_MTS_4.01";
/// Leading magic of Z3D models that are misnamed `.MTS`.
const MAGIC_Z3DM: &[u8] = b"Z3DM";

/// Size of a material record excluding its texture stages.
#[cfg(test)]
const MATERIAL_BASE_SIZE: usize = 112;
/// Size of a texture stage with `frames <= 1`.
const STAGE_BASE_SIZE: usize = 80;
/// Size of the geometry header at offset G.
const GEOMETRY_HEADER_SIZE: usize = 328;
/// Size of one group record.
const GROUP_RECORD_SIZE: usize = 64;
/// Size of one face record.
const FACE_RECORD_SIZE: usize = 36;
/// Stride of a standard vertex.
const VERTEX_STRIDE_48: u32 = 48;
/// Stride of a vertex with the trailing unknown block.
const VERTEX_STRIDE_64: u32 = 64;
/// Geometry flag selecting 64-byte vertices.
const GEOM_FLAG_64: u32 = 0x80_0000;
/// Upper bound on the material count.
const MAX_MATERIALS: u32 = 4096;
/// Upper bound on texture stages per material.
const MAX_STAGES: u32 = 16;
/// Upper bound on an animation table entry count.
const MAX_ANIMATION: u32 = 4096;

/// A parsed MTS model.
#[derive(Debug, Clone)]
pub struct Mts {
    pub materials: Vec<Material>,
    pub groups: Vec<Group>,
    pub vertex_stride: u32,
    /// Geometry-header position (G+256): car-space placement added to every
    /// vertex. Body meshes have `(0, 0, 0)`; wheels, spindles, helmet, driver
    /// and steering wheel carry real values.
    pub position: [f32; 3],
}

/// One material with its texture stages.
#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub colors: [[f32; 4]; 4],
    pub src_blend: u16,
    pub dst_blend: u16,
    pub specular_power: f32,
    pub stages: Vec<TextureStage>,
}

/// One texture stage of a material.
#[derive(Debug, Clone)]
pub struct TextureStage {
    pub texture: String,
    pub frames: u32,
}

/// One geometry group: vertices plus the triangles that reference them.
#[derive(Debug, Clone)]
pub struct Group {
    pub flags: u32,
    pub material_index: u32,
    pub vertices: Vec<Vertex>,
    pub triangles: Vec<[u16; 3]>,
}

/// One vertex in the standard 48-byte layout (extra bytes ignored).
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: u32,
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
}

/// Errors produced while parsing an MTS model.
#[derive(Debug)]
pub enum MtsError {
    BadMagic,
    Unsupported { version: String },
    Truncated { what: &'static str },
    OutOfBounds { what: &'static str },
    BadIndex { group: usize },
    BadReference { what: &'static str },
}

impl fmt::Display for MtsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtsError::BadMagic => write!(f, "bad magic"),
            MtsError::Unsupported { version } => {
                write!(f, "unsupported MTS version {version}")
            }
            MtsError::Truncated { what } => write!(f, "truncated {what}"),
            MtsError::OutOfBounds { what } => write!(f, "out of bounds: {what}"),
            MtsError::BadIndex { group } => {
                write!(f, "vertex index out of range in group {group}")
            }
            MtsError::BadReference { what } => write!(f, "bad reference: {what}"),
        }
    }
}

impl std::error::Error for MtsError {}

/// Parse an MTS model from raw bytes.
pub fn parse(bytes: &[u8]) -> Result<Mts, MtsError> {
    if bytes.len() < MAGIC_410.len() || bytes[..MAGIC_410.len()] != MAGIC_410 {
        if contains(bytes, MAGIC_401) {
            return Err(MtsError::Unsupported {
                version: "4.01".to_string(),
            });
        }
        if bytes.starts_with(MAGIC_Z3DM) {
            return Err(MtsError::Unsupported {
                version: "Z3DM".to_string(),
            });
        }
        return Err(MtsError::BadMagic);
    }

    let mut cursor = Cursor::new(bytes, MAGIC_410.len());
    let material_count = cursor.u32("material_count")?;
    if material_count > MAX_MATERIALS {
        return Err(MtsError::OutOfBounds {
            what: "material_count",
        });
    }

    let mut materials = Vec::with_capacity(material_count as usize);
    for _ in 0..material_count {
        materials.push(read_material(&mut cursor)?);
    }

    let geometry = cursor.pos();
    if geometry + GEOMETRY_MAGIC.len() > bytes.len()
        || &bytes[geometry..geometry + GEOMETRY_MAGIC.len()] != GEOMETRY_MAGIC
    {
        return Err(MtsError::BadMagic);
    }

    let header = region(
        bytes,
        geometry,
        0,
        1,
        GEOMETRY_HEADER_SIZE,
        "geometry header",
    )?;
    let geom_flags = u32_at(header, 20);
    let group_count = u32_at(header, 280) as usize;
    let group_offset = u32_at(header, 284);
    let face_count = u32_at(header, 288) as usize;
    let face_offset = u32_at(header, 292);
    let pair_y_count = u32_at(header, 304);
    let position = [
        f32_at(header, 256),
        f32_at(header, 260),
        f32_at(header, 264),
    ];
    let vertex_stride = if geom_flags & GEOM_FLAG_64 != 0 || pair_y_count > 0 {
        VERTEX_STRIDE_64
    } else {
        VERTEX_STRIDE_48
    };

    let group_bytes = region(
        bytes,
        geometry,
        group_offset,
        group_count,
        GROUP_RECORD_SIZE,
        "group records",
    )?;

    let mut groups = Vec::with_capacity(group_count);
    for index in 0..group_count {
        let record = &group_bytes[index * GROUP_RECORD_SIZE..(index + 1) * GROUP_RECORD_SIZE];
        let flags = u32_at(record, 0);
        let material_index = u32_at(record, 4);
        let vertex_count = u32_at(record, 8) as usize;
        let vertex_offset = u32_at(record, 12);

        if material_index as usize >= materials.len() {
            return Err(MtsError::BadReference {
                what: "group material_index",
            });
        }

        let vertex_bytes = region(
            bytes,
            geometry,
            vertex_offset,
            vertex_count,
            vertex_stride as usize,
            "vertices",
        )?;
        let mut vertices = Vec::with_capacity(vertex_count);
        for vertex in 0..vertex_count {
            let base = vertex * vertex_stride as usize;
            let slice = &vertex_bytes[base..base + VERTEX_STRIDE_48 as usize];
            vertices.push(Vertex {
                position: [f32_at(slice, 0), f32_at(slice, 4), f32_at(slice, 8)],
                normal: [f32_at(slice, 12), f32_at(slice, 16), f32_at(slice, 20)],
                color: u32_at(slice, 24),
                uv0: [f32_at(slice, 32), f32_at(slice, 36)],
                uv1: [f32_at(slice, 40), f32_at(slice, 44)],
            });
        }

        groups.push(Group {
            flags,
            material_index,
            vertices,
            triangles: Vec::new(),
        });
    }

    let face_bytes = region(
        bytes,
        geometry,
        face_offset,
        face_count,
        FACE_RECORD_SIZE,
        "face records",
    )?;
    for index in 0..face_count {
        let record = &face_bytes[index * FACE_RECORD_SIZE..(index + 1) * FACE_RECORD_SIZE];
        let i0 = u16_at(record, 0) as u32;
        let i1 = u16_at(record, 2) as u32;
        let i2 = u16_at(record, 4) as u32;
        let group_index = u32_at(record, 8) as usize;

        if group_index >= groups.len() {
            return Err(MtsError::BadReference {
                what: "face group_index",
            });
        }

        let vertex_count = groups[group_index].vertices.len() as u32;
        if i0 >= vertex_count || i1 >= vertex_count || i2 >= vertex_count {
            return Err(MtsError::BadIndex { group: group_index });
        }
        groups[group_index]
            .triangles
            .push([i0 as u16, i1 as u16, i2 as u16]);
    }

    Ok(Mts {
        materials,
        groups,
        vertex_stride,
        position,
    })
}

/// Sequential bounds-checked reader for the header and material walk.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn pos(&self) -> usize {
        self.pos
    }

    fn take(&mut self, len: usize, what: &'static str) -> Result<&'a [u8], MtsError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(MtsError::Truncated { what })?;
        if end > self.data.len() {
            return Err(MtsError::Truncated { what });
        }
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, MtsError> {
        let bytes = self.take(4, what)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u16(&mut self, what: &'static str) -> Result<u16, MtsError> {
        let bytes = self.take(2, what)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn f32(&mut self, what: &'static str) -> Result<f32, MtsError> {
        let bytes = self.take(4, what)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

fn read_material(cursor: &mut Cursor<'_>) -> Result<Material, MtsError> {
    let name = decode_name(cursor.take(32, "material name")?);
    let _flags = cursor.u32("material flags")?;

    let mut colors = [[0.0f32; 4]; 4];
    for color in colors.iter_mut() {
        for channel in color.iter_mut() {
            *channel = cursor.f32("material color")?;
        }
    }

    let src_blend = cursor.u16("src_blend")?;
    let dst_blend = cursor.u16("dst_blend")?;
    let specular_power = cursor.f32("specular_power")?;
    let stage_count = cursor.u32("stage_count")?;
    if stage_count > MAX_STAGES {
        return Err(MtsError::OutOfBounds {
            what: "stage_count",
        });
    }

    let mut stages = Vec::with_capacity(stage_count as usize);
    for _ in 0..stage_count {
        stages.push(read_stage(cursor)?);
    }

    Ok(Material {
        name,
        colors,
        src_blend,
        dst_blend,
        specular_power,
        stages,
    })
}

fn read_stage(cursor: &mut Cursor<'_>) -> Result<TextureStage, MtsError> {
    let texture = decode_name(cursor.take(32, "texture name")?);
    let _field_a = cursor.u32("stage field")?;
    let _field_b = cursor.u32("stage field")?;
    let frames = cursor.u32("frames")?;

    if frames > 1 {
        let n = cursor.u32("animation count")?;
        if n > MAX_ANIMATION {
            return Err(MtsError::OutOfBounds {
                what: "animation count",
            });
        }
        let animated = (n as usize).checked_mul(4).ok_or(MtsError::OutOfBounds {
            what: "animation table",
        })?;
        cursor.take(animated, "animation table")?;
        cursor.take(8, "animation table")?;
    }

    let _params = cursor.take(STAGE_BASE_SIZE - 44, "stage parameters")?;
    Ok(TextureStage { texture, frames })
}

/// Return the slice `[offset, offset + count * size)` relative to `base`,
/// checking every arithmetic step against the buffer length.
fn region<'a>(
    bytes: &'a [u8],
    base: usize,
    offset: u32,
    count: usize,
    size: usize,
    what: &'static str,
) -> Result<&'a [u8], MtsError> {
    let start = base
        .checked_add(offset as usize)
        .ok_or(MtsError::OutOfBounds { what })?;
    let len = count
        .checked_mul(size)
        .ok_or(MtsError::OutOfBounds { what })?;
    let end = start
        .checked_add(len)
        .ok_or(MtsError::OutOfBounds { what })?;
    if end > bytes.len() {
        return Err(MtsError::OutOfBounds { what });
    }
    Ok(&bytes[start..end])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Decode a NUL-terminated Latin-1 name (each byte maps to one `char`).
fn decode_name(raw: &[u8]) -> String {
    let mut name = String::new();
    for &byte in raw {
        if byte == 0 {
            break;
        }
        name.push(byte as char);
    }
    name
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRIDE_48: usize = 48;
    const STRIDE_64: usize = 64;

    struct TestGroup {
        flags: u32,
        material_index: u32,
        vertices: Vec<Vertex>,
        triangles: Vec<[u16; 3]>,
        face_group_index: Option<u32>,
    }

    fn name32(name: &str) -> [u8; 32] {
        let mut raw = [0u8; 32];
        raw[..name.len()].copy_from_slice(name.as_bytes());
        raw
    }

    fn test_vertex() -> Vertex {
        Vertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            color: 0xFF00_FF00,
            uv0: [0.25, 0.75],
            uv1: [0.5, 0.5],
        }
    }

    fn stage(texture: &str, frames: u32, animation: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&name32(texture));
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&frames.to_le_bytes());
        if frames > 1 {
            out.extend_from_slice(&(animation.len() as u32).to_le_bytes());
            for value in animation {
                out.extend_from_slice(&value.to_le_bytes());
            }
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; STAGE_BASE_SIZE - 44]);
        out
    }

    fn material(name: &str, stages: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&name32(name));
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&[0u8; 64]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&(stages.len() as u32).to_le_bytes());
        for bytes in stages {
            out.extend_from_slice(bytes);
        }
        assert_eq!(
            out.len(),
            MATERIAL_BASE_SIZE + stages.iter().map(Vec::len).sum::<usize>()
        );
        out
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn build(
        materials: &[Vec<u8>],
        geom_flags: u32,
        pair_y_count: u32,
        groups: &[TestGroup],
        stride: usize,
    ) -> Vec<u8> {
        let mut geo = vec![0u8; GEOMETRY_HEADER_SIZE];
        let group_offset = geo.len() as u32;
        geo.extend(std::iter::repeat_n(0u8, GROUP_RECORD_SIZE * groups.len()));

        let mut vertex_offsets = Vec::new();
        for group in groups {
            vertex_offsets.push(geo.len() as u32);
            for vertex in &group.vertices {
                for value in vertex.position.iter().chain(vertex.normal.iter()) {
                    geo.extend_from_slice(&value.to_le_bytes());
                }
                geo.extend_from_slice(&vertex.color.to_le_bytes());
                geo.extend_from_slice(&0u32.to_le_bytes());
                for value in vertex.uv0.iter().chain(vertex.uv1.iter()) {
                    geo.extend_from_slice(&value.to_le_bytes());
                }
                if stride == STRIDE_64 {
                    geo.extend_from_slice(&[0u8; 16]);
                }
            }
        }

        let face_offset = geo.len() as u32;
        let total_faces: u32 = groups
            .iter()
            .map(|group| group.triangles.len() as u32)
            .sum();
        for (group_index, group) in groups.iter().enumerate() {
            let written_index = group.face_group_index.unwrap_or(group_index as u32);
            for triangle in &group.triangles {
                geo.extend_from_slice(&triangle[0].to_le_bytes());
                geo.extend_from_slice(&triangle[1].to_le_bytes());
                geo.extend_from_slice(&triangle[2].to_le_bytes());
                geo.extend_from_slice(&0u16.to_le_bytes());
                geo.extend_from_slice(&written_index.to_le_bytes());
                geo.extend_from_slice(&[0u8; 24]);
            }
        }

        geo[0..16].copy_from_slice(&MAGIC_410);
        write_u32(&mut geo, 20, geom_flags);
        write_u32(&mut geo, 280, groups.len() as u32);
        write_u32(&mut geo, 284, group_offset);
        write_u32(&mut geo, 288, total_faces);
        write_u32(&mut geo, 292, face_offset);
        write_u32(&mut geo, 304, pair_y_count);

        for (index, group) in groups.iter().enumerate() {
            let base = group_offset as usize + index * GROUP_RECORD_SIZE;
            write_u32(&mut geo, base, group.flags);
            write_u32(&mut geo, base + 4, group.material_index);
            write_u32(&mut geo, base + 8, group.vertices.len() as u32);
            write_u32(&mut geo, base + 12, vertex_offsets[index]);
            write_u32(&mut geo, base + 16, 0);
            write_u32(&mut geo, base + 20, 0);
        }

        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC_410);
        out.extend_from_slice(&(materials.len() as u32).to_le_bytes());
        for record in materials {
            out.extend_from_slice(record);
        }
        out.extend_from_slice(&geo);
        out
    }

    fn two_triangle_group() -> TestGroup {
        TestGroup {
            flags: 0x82,
            material_index: 0,
            vertices: vec![test_vertex(); 4],
            triangles: vec![[0, 1, 2], [1, 2, 3]],
            face_group_index: None,
        }
    }

    #[test]
    fn parses_one_material_one_group() {
        let bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[two_triangle_group()],
            STRIDE_48,
        );
        let mts = parse(&bytes).unwrap();
        assert_eq!(mts.vertex_stride, 48);
        assert_eq!(mts.materials.len(), 1);
        assert_eq!(mts.materials[0].name, "BODY");
        assert_eq!(mts.materials[0].src_blend, 1);
        assert_eq!(mts.materials[0].dst_blend, 2);
        assert_eq!(mts.materials[0].stages.len(), 1);
        assert_eq!(mts.materials[0].stages[0].texture, "HUB.BMP");
        assert_eq!(mts.materials[0].stages[0].frames, 1);
        assert_eq!(mts.groups.len(), 1);
        assert_eq!(mts.groups[0].vertices.len(), 4);
        assert_eq!(mts.groups[0].vertices[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(mts.groups[0].vertices[0].color, 0xFF00_FF00);
        assert_eq!(mts.groups[0].triangles, vec![[0, 1, 2], [1, 2, 3]]);
    }

    #[test]
    fn parses_animated_stage() {
        let bytes = build(
            &[material("WHEEL", &[stage("WHEEL.BMP", 3, &[10, 11, 12])])],
            0,
            0,
            &[two_triangle_group()],
            STRIDE_48,
        );
        let mts = parse(&bytes).unwrap();
        assert_eq!(mts.materials[0].stages[0].frames, 3);
    }

    #[test]
    fn parses_64_byte_vertex_flag() {
        let bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            GEOM_FLAG_64,
            0,
            &[two_triangle_group()],
            STRIDE_64,
        );
        let mts = parse(&bytes).unwrap();
        assert_eq!(mts.vertex_stride, 64);
        assert_eq!(mts.groups[0].vertices[0].uv1, [0.5, 0.5]);
    }

    #[test]
    fn parses_64_byte_vertex_from_pair_y() {
        let bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            1,
            &[two_triangle_group()],
            STRIDE_64,
        );
        let mts = parse(&bytes).unwrap();
        assert_eq!(mts.vertex_stride, 64);
    }

    #[test]
    fn detects_legacy_4_01() {
        let mut bytes = vec![0u8; 64];
        bytes[4..4 + MAGIC_401.len()].copy_from_slice(MAGIC_401);
        assert!(matches!(
            parse(&bytes),
            Err(MtsError::Unsupported { ref version }) if version == "4.01"
        ));
    }

    #[test]
    fn rejects_bad_magic() {
        let bytes = vec![0u8; 64];
        assert!(matches!(parse(&bytes), Err(MtsError::BadMagic)));
    }

    #[test]
    fn detects_misnamed_z3dm() {
        let mut bytes = vec![0u8; 64];
        bytes[..4].copy_from_slice(b"Z3DM");
        assert!(matches!(
            parse(&bytes),
            Err(MtsError::Unsupported { ref version }) if version == "Z3DM"
        ));
    }

    #[test]
    fn accepts_nonzero_geometry_padding() {
        let mut bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[two_triangle_group()],
            STRIDE_48,
        );
        let geometry = bytes.len()
            - (GEOMETRY_HEADER_SIZE + GROUP_RECORD_SIZE + 4 * STRIDE_48 + 2 * FACE_RECORD_SIZE);
        bytes[geometry + 14] = 0x80;
        bytes[geometry + 15] = 0x3F;
        assert!(parse(&bytes).is_ok());
    }

    #[test]
    fn reads_geometry_position_at_256() {
        let mut bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[two_triangle_group()],
            STRIDE_48,
        );
        let geometry = 16
            + bytes[16..]
                .windows(GEOMETRY_MAGIC.len())
                .position(|window| window == GEOMETRY_MAGIC)
                .unwrap();
        for (index, value) in [-0.82f32, 0.49, 1.25].iter().enumerate() {
            let at = geometry + 256 + index * 4;
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mts = parse(&bytes).unwrap();
        assert_eq!(mts.position, [-0.82, 0.49, 1.25]);
    }

    #[test]
    fn rejects_truncated_material() {
        let mut bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[two_triangle_group()],
            STRIDE_48,
        );
        bytes.truncate(MAGIC_410.len() + 4 + 10);
        assert!(matches!(parse(&bytes), Err(MtsError::Truncated { .. })));
    }

    #[test]
    fn rejects_face_index_out_of_range() {
        let mut group = two_triangle_group();
        group.triangles = vec![[0, 1, 9]];
        let bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[group],
            STRIDE_48,
        );
        assert!(matches!(
            parse(&bytes),
            Err(MtsError::BadIndex { group: 0 })
        ));
    }

    #[test]
    fn rejects_group_index_out_of_range() {
        let mut group = two_triangle_group();
        group.face_group_index = Some(9);
        let bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[group],
            STRIDE_48,
        );
        assert!(matches!(
            parse(&bytes),
            Err(MtsError::BadReference {
                what: "face group_index"
            })
        ));
    }

    #[test]
    fn rejects_material_index_out_of_range() {
        let mut group = two_triangle_group();
        group.material_index = 7;
        let bytes = build(
            &[material("BODY", &[stage("HUB.BMP", 1, &[])])],
            0,
            0,
            &[group],
            STRIDE_48,
        );
        assert!(matches!(
            parse(&bytes),
            Err(MtsError::BadReference {
                what: "group material_index"
            })
        ));
    }
}
