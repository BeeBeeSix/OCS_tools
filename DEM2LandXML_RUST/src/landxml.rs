use crate::dem::{point_in_poly, DemData};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ExportMode {
    Both,
    XmlOnly,
    CsvOnly,
    Tin12daOnly,
    Tin12dazOnly,
    All,
}

#[derive(Clone, Debug, Default)]
pub struct Tin12daOptions {
    pub tin_name: Option<String>,
    pub colour: Option<String>,
    pub null_length: Option<f64>,
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn write_12da_tin<W: Write>(
    w: &mut W,
    surf_name: &str,
    pts: &[(usize, f64, f64, f64)],
    faces: &[(usize, usize, usize)],
    tin_opts: Option<&Tin12daOptions>,
) -> Result<(), String> {
    let default_opts = Tin12daOptions::default();
    let opts = tin_opts.unwrap_or(&default_opts);
    let resolved_name = opts
        .tin_name
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(surf_name);

    writeln!(w, "tin {{").map_err(|e| e.to_string())?;
    writeln!(w, "  name \"{resolved_name}\"").map_err(|e| e.to_string())?;
    if let Some(colour) = &opts.colour {
        if !colour.trim().is_empty() {
            writeln!(w, "  colour \"{colour}\"").map_err(|e| e.to_string())?;
        }
    }
    if let Some(null_len) = opts.null_length {
        if null_len > 0.0 {
            writeln!(w, "  null_length {}", fmt(null_len)).map_err(|e| e.to_string())?;
        }
    }
    writeln!(w, "  points {{").map_err(|e| e.to_string())?;
    for &(_, x, y, z) in pts {
        writeln!(w, "    {} {} {}", fmt(x), fmt(y), fmt(z)).map_err(|e| e.to_string())?;
    }
    writeln!(w, "  }}").map_err(|e| e.to_string())?;
    writeln!(w, "  triangles {{").map_err(|e| e.to_string())?;
    for &(p1, p2, p3) in faces {
        writeln!(w, "    {p1} {p2} {p3}").map_err(|e| e.to_string())?;
    }
    writeln!(w, "  }}").map_err(|e| e.to_string())?;
    writeln!(w, "}}").map_err(|e| e.to_string())?;
    Ok(())
}
pub fn export_dem(
    dem: &DemData,
    out_path: &Path,
    surf_name: &str,
    step: usize,
    poly: &[(f64, f64)],
    min_z: Option<f32>,
    max_z: Option<f32>,
    mode: ExportMode,
    tin_opts: Option<&Tin12daOptions>,
) -> Result<usize, String> {
    let rows = dem.height.div_ceil(step);
    let cols = dem.width.div_ceil(step);
    let mut ids = vec![0usize; rows * cols];
    let mut pts = Vec::new();

    for (gr, r) in (0..dem.height).step_by(step).enumerate() {
        for (gc, c) in (0..dem.width).step_by(step).enumerate() {
            let z = dem.data[r * dem.width + c];
            if !dem.is_valid(z, min_z, max_z) { continue; }
            let (gx, gy) = dem.pixel_center_geo(c, r);
            if poly.len() >= 3 && !point_in_poly(gx, gy, poly) { continue; }
            let id = pts.len() + 1;
            ids[gr * cols + gc] = id;
            pts.push((id, gx, gy, z as f64));
        }
    }
    if pts.is_empty() {
        return Err("No valid points found with current filters.".into());
    }

    let mut faces = Vec::new();
    for r in 0..rows.saturating_sub(1) {
        for c in 0..cols.saturating_sub(1) {
            let (p1, p2) = (ids[r * cols + c], ids[r * cols + c + 1]);
            let (p3, p4) = (ids[(r + 1) * cols + c], ids[(r + 1) * cols + c + 1]);
            if p1 > 0 && p2 > 0 && p3 > 0 { faces.push((p1, p2, p3)); }
            if p2 > 0 && p4 > 0 && p3 > 0 { faces.push((p2, p4, p3)); }
        }
    }

    // 1. CSV Export
    if matches!(mode, ExportMode::Both | ExportMode::CsvOnly | ExportMode::All) {
        let csv_path = out_path.with_extension("csv");
        let mut cw = BufWriter::new(File::create(csv_path).map_err(|e| e.to_string())?);
        writeln!(cw, "Easting,Northing,Elevation").map_err(|e| e.to_string())?;
        for &(_, x, y, z) in &pts {
            writeln!(cw, "{},{},{}", fmt(x), fmt(y), fmt(z)).map_err(|e| e.to_string())?;
        }
    }

    // 2. 12da TIN Export (uncompressed)
    if matches!(mode, ExportMode::Tin12daOnly | ExportMode::All) {
        let da_path = out_path.with_extension("12da");
        let mut dw = BufWriter::new(File::create(da_path).map_err(|e| e.to_string())?);
        write_12da_tin(&mut dw, surf_name, &pts, &faces, tin_opts)?;
    }

    // 3. 12daz TIN Export (zipped .12da)
    if matches!(mode, ExportMode::Tin12dazOnly | ExportMode::All) {
        let daz_path = out_path.with_extension("12daz");
        let inner_name = out_path
            .with_extension("12da")
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("surface.12da")
            .to_string();
        let file = File::create(daz_path).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(BufWriter::new(file));
        let zip_opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(&inner_name, zip_opts).map_err(|e| e.to_string())?;
        write_12da_tin(&mut zip, surf_name, &pts, &faces, tin_opts)?;
        zip.finish().map_err(|e| e.to_string())?;
    }

    // 4. LandXML Export
    if matches!(mode, ExportMode::Both | ExportMode::XmlOnly | ExportMode::All) {
        let mut w = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
        writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#).ok();
        writeln!(w, r#"<LandXML xmlns="http://landxml.org" version="1.2" date="2026-11-25" time="00:00:00" readOnly="false">"#).ok();
        writeln!(w, r#"  <Units><Metric areaUnit="squareMeter" linearUnit="meter" volumeUnit="cubicMeter" temperatureUnit="celsius" pressureUnit="milliBars" diameterUnit="millimeter" angularUnit="decimal degrees" directionUnit="decimal degrees"/></Units>"#).ok();
        writeln!(w, "  <CgPoints>").ok();
        for &(id, x, y, z) in &pts {
            writeln!(w, r#"    <CgPoint name="{id}" code="DEM">{} {} {}</CgPoint>"#, fmt(y), fmt(x), fmt(z)).ok();
        }
        writeln!(w, "  </CgPoints>\n  <Surfaces>\n    <Surface name=\"{surf_name}\">\n      <Definition surfType=\"TIN\">\n        <Pnts>").ok();
        for &(id, x, y, z) in &pts {
            writeln!(w, r#"          <P id="{id}">{} {} {}</P>"#, fmt(y), fmt(x), fmt(z)).ok();
        }
        writeln!(w, "        </Pnts>\n        <Faces>").ok();
        for &(p1, p2, p3) in &faces {
            writeln!(w, "          <F>{p1} {p2} {p3}</F>").ok();
        }
        writeln!(w, "        </Faces>\n      </Definition>\n    </Surface>\n  </Surfaces>\n</LandXML>").map_err(|e| e.to_string())?;
    }

    Ok(pts.len())
}
