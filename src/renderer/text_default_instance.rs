//! Shaping faces of variable fonts at their default instance.
//!
//! RUI never sets variation coordinates, so every face is shaped and measured
//! at its default instance, where the variation tables change nothing. Yet for
//! a TrueType variable font without `HVAR`, such as ThonburiUI, ttf-parser reads
//! each glyph's `gvar` deltas to find its advance, and decodes its outline to
//! find its box. A face parsed without those tables answers the same from
//! `hmtx` and `glyf` alone.

use rustybuzz::Face;
use rustybuzz::ttf_parser::{self, RawFace, RawFaceTables};

/// Face `index` of `data`, without its variation tables when its advances
/// come from `gvar` (no `HVAR`) and its outlines do not need them (`CFF2`
/// blends at any instance).
pub(crate) fn default_instance_face(data: &[u8], index: u32) -> Option<Face<'_>> {
    let face = Face::from_slice(data, index)?;
    let tables = face.tables();
    if !face.is_variable() || tables.hvar.is_some() || tables.cff2.is_some() {
        return Some(face);
    }
    let raw = RawFace::parse(data, index).ok()?;
    let mut tables = RawFaceTables::default();
    for record in raw.table_records {
        let start = record.offset as usize;
        let table = start
            .checked_add(record.length as usize)
            .and_then(|end| data.get(start..end));
        let Some(table) = table else {
            continue;
        };
        let slot = match &record.tag.to_bytes() {
            b"head" => {
                tables.head = table;
                continue;
            }
            b"hhea" => {
                tables.hhea = table;
                continue;
            }
            b"maxp" => {
                tables.maxp = table;
                continue;
            }
            b"bdat" => &mut tables.bdat,
            b"bloc" => &mut tables.bloc,
            b"CBDT" => &mut tables.cbdt,
            b"CBLC" => &mut tables.cblc,
            b"CFF " => &mut tables.cff,
            b"COLR" => &mut tables.colr,
            b"CPAL" => &mut tables.cpal,
            b"EBDT" => &mut tables.ebdt,
            b"EBLC" => &mut tables.eblc,
            b"GDEF" => &mut tables.gdef,
            b"GPOS" => &mut tables.gpos,
            b"GSUB" => &mut tables.gsub,
            b"MATH" => &mut tables.math,
            b"OS/2" => &mut tables.os2,
            b"SVG " => &mut tables.svg,
            b"VORG" => &mut tables.vorg,
            b"ankr" => &mut tables.ankr,
            b"cmap" => &mut tables.cmap,
            b"feat" => &mut tables.feat,
            b"glyf" => &mut tables.glyf,
            b"hmtx" => &mut tables.hmtx,
            b"kern" => &mut tables.kern,
            b"kerx" => &mut tables.kerx,
            b"loca" => &mut tables.loca,
            b"morx" => &mut tables.morx,
            b"name" => &mut tables.name,
            b"post" => &mut tables.post,
            b"sbix" => &mut tables.sbix,
            b"STAT" => &mut tables.stat,
            b"trak" => &mut tables.trak,
            b"vhea" => &mut tables.vhea,
            b"vmtx" => &mut tables.vmtx,
            // `fvar`, `gvar`, `avar`, `HVAR`, `MVAR` and `VVAR` are left out.
            _ => continue,
        };
        *slot = Some(table);
    }
    let face = ttf_parser::Face::from_raw_tables(tables).ok()?;
    Some(Face::from_face(face))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustybuzz::ttf_parser::GlyphId;

    #[cfg(target_os = "macos")]
    #[test]
    fn a_variable_face_keeps_its_default_advances_and_boxes() {
        for path in [
            "/System/Library/Fonts/ThonburiUI.ttc",
            "/System/Library/Fonts/SFNS.ttf",
        ] {
            let Ok(data) = std::fs::read(path) else {
                continue;
            };
            let full = Face::from_slice(&data, 0).expect("face");
            let plain = default_instance_face(&data, 0).expect("face");
            if plain.is_variable() {
                assert!(!path.contains("Thonburi"), "{path} keeps its variations");
                continue;
            }
            for id in (0..full.number_of_glyphs()).map(GlyphId) {
                let advance = (full.glyph_hor_advance(id), plain.glyph_hor_advance(id));
                assert_eq!(advance.0, advance.1, "{path} glyph {id:?}");
                let bounds = (full.glyph_bounding_box(id), plain.glyph_bounding_box(id));
                assert_eq!(bounds.0, bounds.1, "{path} glyph {id:?}");
            }
        }
    }
}
