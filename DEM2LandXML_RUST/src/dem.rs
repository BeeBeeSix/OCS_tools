use std::fs::File;
use std::path::Path;
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

#[derive(Clone)]
pub struct DemData {
    pub width: usize,
    pub height: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub pixel_x: f64,
    pub pixel_y: f64,
    pub nodata: Option<f32>,
    pub data: Vec<f32>,
}

fn read_f64_tag(dec: &mut Decoder<File>, tag_id: u16) -> Option<Vec<f64>> {
    use tiff::decoder::ifd::Value;
    match dec.get_tag(Tag::Unknown(tag_id)).ok()? {
        Value::Double(v) => Some(vec![v]),
        Value::Float(v) => Some(vec![v as f64]),
        Value::List(list) => Some(list.into_iter().filter_map(|x| match x {
            Value::Double(v) => Some(v),
            Value::Float(v) => Some(v as f64),
            _ => None,
        }).collect()),
        _ => dec.get_tag_f64_vec(Tag::Unknown(tag_id)).ok(),
    }
}

fn read_world_file(tif_path: &Path) -> Option<(f64, f64, f64, f64)> {
    for ext in ["tfw", "tifw", "wld", "TFW", "TIFW"] {
        let wp = tif_path.with_extension(ext);
        if let Ok(txt) = std::fs::read_to_string(wp) {
            let nums: Vec<f64> = txt.lines().filter_map(|l| l.trim().parse().ok()).collect();
            if nums.len() >= 6 {
                let (px, py) = (nums.first().copied()?, nums.get(3).copied()?.abs());
                let (cx, cy) = (nums.get(4).copied()?, nums.get(5).copied()?);
                // World file lines 5 & 6 are top-left pixel CENTER; convert to top-left CORNER
                return Some((cx - 0.5 * px, cy + 0.5 * py, px, py));
            }
        }
    }
    None
}

impl DemData {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mut dec = Decoder::new(file).map_err(|e| e.to_string())?;
        let (w, h) = dec.dimensions().map_err(|e| e.to_string())?;

        let (origin_x, origin_y, pixel_x, pixel_y) =
            if let Some(m) = read_f64_tag(&mut dec, 34264).filter(|v| v.len() >= 8) {
                (m.get(3).copied().unwrap_or(0.0), m.get(7).copied().unwrap_or(0.0),
                 m.first().copied().unwrap_or(1.0), m.get(5).copied().unwrap_or(1.0).abs())
            } else if let (Some(scale), Some(tie)) = (read_f64_tag(&mut dec, 33550), read_f64_tag(&mut dec, 33922)) {
                let (px, py) = (scale.first().copied().unwrap_or(1.0), scale.get(1).copied().unwrap_or(1.0).abs());
                let (i, j) = (tie.first().copied().unwrap_or(0.0), tie.get(1).copied().unwrap_or(0.0));
                let (x, y) = (tie.get(3).copied().unwrap_or(0.0), tie.get(4).copied().unwrap_or(0.0));
                (x - i * px, y + j * py, px, py)
            } else if let Some(wf) = read_world_file(path) {
                wf
            } else {
                return Err("No GeoTIFF tags (33922/33550/34264) or .tfw world file found.".into());
            };


        let nodata = dec
            .get_tag_ascii_string(Tag::Unknown(42113))
            .ok()
            .and_then(|s| s.trim_matches('\0').trim().parse::<f32>().ok());

        let img = dec.read_image().map_err(|e| e.to_string())?;
        let data: Vec<f32> = match img {
            DecodingResult::F32(v) => v,
            DecodingResult::F64(v) => v.into_iter().map(|x| x as f32).collect(),
            DecodingResult::I16(v) => v.into_iter().map(|x| x as f32).collect(),
            DecodingResult::U16(v) => v.into_iter().map(|x| x as f32).collect(),
            _ => return Err("Unsupported GeoTIFF sample format.".into()),
        };

        Ok(Self { width: w as usize, height: h as usize, origin_x, origin_y, pixel_x, pixel_y, nodata, data })
    }

    pub fn is_valid(&self, z: f32, min_z: Option<f32>, max_z: Option<f32>) -> bool {
        if !z.is_finite() || z <= -9000.0 || z >= 1e10 { return false; }
        if let Some(nd) = self.nodata {
            if (z - nd).abs() < 1e-4 { return false; }
        }
        if let Some(min) = min_z { if z < min { return false; } }
        if let Some(max) = max_z { if z > max { return false; } }
        true
    }

    pub fn pixel_center_geo(&self, col: usize, row: usize) -> (f64, f64) {
        (
            self.origin_x + (col as f64 + 0.5) * self.pixel_x,
            self.origin_y - (row as f64 + 0.5) * self.pixel_y,
        )
    }

    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        (
            self.origin_x,
            self.origin_y - self.height as f64 * self.pixel_y,
            self.origin_x + self.width as f64 * self.pixel_x,
            self.origin_y,
        )
    }

    pub fn estimate_count(&self, step: usize, poly: &[(f64, f64)], min_z: Option<f32>, max_z: Option<f32>) -> usize {
        let mut count = 0;
        for r in (0..self.height).step_by(step) {
            for c in (0..self.width).step_by(step) {
                let z = self.data[r * self.width + c];
                if !self.is_valid(z, min_z, max_z) { continue; }
                if poly.len() >= 3 && !point_in_poly(self.pixel_center_geo(c, r).0, self.pixel_center_geo(c, r).1, poly) { continue; }
                count += 1;
            }
        }
        count
    }
}

pub fn point_in_poly(x: f64, y: f64, poly: &[(f64, f64)]) -> bool {
    let (mut inside, mut j) = (false, poly.len() - 1);
    for i in 0..poly.len() {
        let ((xi, yi), (xj, yj)) = (poly[i], poly[j]);
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi + 1e-15) + xi) { inside = !inside; }
        j = i;
    }
    inside
}

pub fn load_boundary(path: &Path) -> Result<Vec<(f64, f64)>, String> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    if ext == "shp" {
        let mut reader = shapefile::Reader::from_path(path).map_err(|e| e.to_string())?;
        for (shape, _) in reader.read().map_err(|e| e.to_string())? {
            if let shapefile::Shape::Polygon(p) = shape {
                if let Some(ring) = p.rings().first() {
                    return Ok(ring.points().iter().map(|pt| (pt.x, pt.y)).collect());
                }
            }
        }
        Err("No polygon found in Shapefile.".into())
    } else {
        let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        let geom = v.get("features").and_then(|f| f.get(0)).and_then(|f| f.get("geometry")).unwrap_or(&v);
        let ring = geom["coordinates"].as_array().ok_or("Invalid GeoJSON polygon")?;
        Ok(ring.iter().filter_map(|p| Some((p.as_f64()?, p.as_f64()?))).collect())
    }
}
