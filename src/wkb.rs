//! Minimal WKB (Well-Known Binary) → WKT (Well-Known Text) converter.
//!
//! Supports the geometry types used by geobr: Point, MultiPoint, LineString,
//! MultiLineString, Polygon, MultiPolygon, GeometryCollection.
//! Handles both little-endian and big-endian byte order.

/// Convert a WKB byte slice to WKT string.
pub fn wkb_to_wkt(wkb: &[u8]) -> Result<String, String> {
    let mut reader = WkbReader::new(wkb);
    reader.read_geometry()
}

struct WkbReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> WkbReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        if self.remaining() < 1 {
            return Err("unexpected end of WKB".into());
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u32(&mut self, le: bool) -> Result<u32, String> {
        if self.remaining() < 4 {
            return Err("unexpected end of WKB".into());
        }
        let bytes = &self.data[self.pos..self.pos + 4];
        let v = if le {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        };
        self.pos += 4;
        Ok(v)
    }

    fn read_f64(&mut self, le: bool) -> Result<f64, String> {
        if self.remaining() < 8 {
            return Err("unexpected end of WKB".into());
        }
        let bytes = &self.data[self.pos..self.pos + 8];
        let v = if le {
            f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        } else {
            f64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        };
        self.pos += 8;
        Ok(v)
    }

    fn read_geometry(&mut self) -> Result<String, String> {
        let byte_order = self.read_u8()?;
        let le = byte_order == 1;
        let geom_type = self.read_u32(le)?;

        match geom_type {
            1 => self.read_point(le),
            2 => self.read_linestring(le),
            3 => self.read_polygon(le),
            4 => self.read_multipoint(le),
            5 => self.read_multilinestring(le),
            6 => self.read_multipolygon(le),
            7 => self.read_geometry_collection(le),
            _ => Err(format!("unsupported geometry type: {geom_type}")),
        }
    }

    fn read_point(&mut self, le: bool) -> Result<String, String> {
        let x = self.read_f64(le)?;
        let y = self.read_f64(le)?;
        Ok(format!("POINT ({x} {y})"))
    }

    fn read_linestring(&mut self, le: bool) -> Result<String, String> {
        let n = self.read_u32(le)? as usize;
        let mut coords = Vec::with_capacity(n);
        for _ in 0..n {
            let x = self.read_f64(le)?;
            let y = self.read_f64(le)?;
            coords.push(format!("{x} {y}"));
        }
        Ok(format!("LINESTRING ({})", coords.join(", ")))
    }

    fn read_polygon(&mut self, le: bool) -> Result<String, String> {
        let n_rings = self.read_u32(le)? as usize;
        let mut rings = Vec::with_capacity(n_rings);
        for _ in 0..n_rings {
            let n_pts = self.read_u32(le)? as usize;
            let mut coords = Vec::with_capacity(n_pts);
            for _ in 0..n_pts {
                let x = self.read_f64(le)?;
                let y = self.read_f64(le)?;
                coords.push(format!("{x} {y}"));
            }
            rings.push(format!("({})", coords.join(", ")));
        }
        Ok(format!("POLYGON ({})", rings.join(", ")))
    }

    fn read_multipoint(&mut self, le: bool) -> Result<String, String> {
        let n = self.read_u32(le)? as usize;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            let wkt = self.read_geometry()?;
            // Extract inner coords from POINT (x y)
            let inner = wkt
                .strip_prefix("POINT (")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(&wkt);
            points.push(format!("({inner})"));
        }
        Ok(format!("MULTIPOINT ({})", points.join(", ")))
    }

    fn read_multilinestring(&mut self, le: bool) -> Result<String, String> {
        let n = self.read_u32(le)? as usize;
        let mut lines = Vec::with_capacity(n);
        for _ in 0..n {
            let wkt = self.read_geometry()?;
            let inner = wkt
                .strip_prefix("LINESTRING (")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(&wkt);
            lines.push(format!("({inner})"));
        }
        Ok(format!("MULTILINESTRING ({})", lines.join(", ")))
    }

    fn read_multipolygon(&mut self, le: bool) -> Result<String, String> {
        let n = self.read_u32(le)? as usize;
        let mut polys = Vec::with_capacity(n);
        for _ in 0..n {
            let wkt = self.read_geometry()?;
            let inner = wkt
                .strip_prefix("POLYGON (")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(&wkt);
            polys.push(format!("({inner})"));
        }
        Ok(format!("MULTIPOLYGON ({})", polys.join(", ")))
    }

    fn read_geometry_collection(&mut self, le: bool) -> Result<String, String> {
        let n = self.read_u32(le)? as usize;
        let mut geoms = Vec::with_capacity(n);
        for _ in 0..n {
            geoms.push(self.read_geometry()?);
        }
        Ok(format!("GEOMETRYCOLLECTION ({})", geoms.join(", ")))
    }
}
