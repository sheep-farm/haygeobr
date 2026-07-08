//! Unit tests for WKB → WKT conversion.
//!
//! WKB bytes are constructed manually following the OGC specification:
//!   byte_order (1 byte) + geom_type (4 bytes) + coordinates
//!
//! Little-endian (byte_order = 1) is the default for geobr data.

use haygeobr::wkb::wkb_to_wkt;

/// Build a little-endian WKB Point.
fn wkb_point(x: f64, y: f64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(21);
    buf.push(1); // little-endian
    buf.extend_from_slice(&1u32.to_le_bytes()); // Point
    buf.extend_from_slice(&x.to_le_bytes());
    buf.extend_from_slice(&y.to_le_bytes());
    buf
}

/// Build a little-endian WKB LineString.
fn wkb_linestring(coords: &[(f64, f64)]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(1); // LE
    buf.extend_from_slice(&2u32.to_le_bytes()); // LineString
    buf.extend_from_slice(&(coords.len() as u32).to_le_bytes());
    for &(x, y) in coords {
        buf.extend_from_slice(&x.to_le_bytes());
        buf.extend_from_slice(&y.to_le_bytes());
    }
    buf
}

/// Build a little-endian WKB Polygon.
fn wkb_polygon(rings: &[Vec<(f64, f64)>]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(1); // LE
    buf.extend_from_slice(&3u32.to_le_bytes()); // Polygon
    buf.extend_from_slice(&(rings.len() as u32).to_le_bytes());
    for ring in rings {
        buf.extend_from_slice(&(ring.len() as u32).to_le_bytes());
        for &(x, y) in ring {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
        }
    }
    buf
}

/// Build a little-endian WKB MultiPoint.
fn wkb_multipoint(points: &[(f64, f64)]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(1); // LE
    buf.extend_from_slice(&4u32.to_le_bytes()); // MultiPoint
    buf.extend_from_slice(&(points.len() as u32).to_le_bytes());
    for &(x, y) in points {
        buf.extend_from_slice(&wkb_point(x, y));
    }
    buf
}

/// Build a little-endian WKB MultiPolygon.
fn wkb_multipolygon(polygons: &[Vec<Vec<(f64, f64)>>]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(1); // LE
    buf.extend_from_slice(&6u32.to_le_bytes()); // MultiPolygon
    buf.extend_from_slice(&(polygons.len() as u32).to_le_bytes());
    for rings in polygons {
        buf.extend_from_slice(&wkb_polygon(rings));
    }
    buf
}

// =========================================================================
// Point
// =========================================================================

#[test]
fn test_point_le() {
    let wkb = wkb_point(1.0, 2.0);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "POINT (1 2)");
}

#[test]
fn test_point_negative_coords() {
    let wkb = wkb_point(-43.2075, -22.9116);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "POINT (-43.2075 -22.9116)");
}

#[test]
fn test_point_be() {
    let mut buf = Vec::with_capacity(21);
    buf.push(0); // big-endian
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.extend_from_slice(&1.0f64.to_be_bytes());
    buf.extend_from_slice(&2.0f64.to_be_bytes());
    let wkt = wkb_to_wkt(&buf).unwrap();
    assert_eq!(wkt, "POINT (1 2)");
}

// =========================================================================
// LineString
// =========================================================================

#[test]
fn test_linestring_two_points() {
    let wkb = wkb_linestring(&[(0.0, 0.0), (1.0, 1.0)]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "LINESTRING (0 0, 1 1)");
}

#[test]
fn test_linestring_three_points() {
    let wkb = wkb_linestring(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "LINESTRING (0 0, 1 1, 2 0)");
}

// =========================================================================
// Polygon
// =========================================================================

#[test]
fn test_polygon_single_ring() {
    let ring = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    let wkb = wkb_polygon(&[ring]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "POLYGON ((0 0, 0 1, 1 1, 1 0, 0 0))");
}

#[test]
fn test_polygon_two_rings() {
    let outer = vec![
        (0.0, 0.0),
        (0.0, 10.0),
        (10.0, 10.0),
        (10.0, 0.0),
        (0.0, 0.0),
    ];
    let inner = vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0), (2.0, 2.0)];
    let wkb = wkb_polygon(&[outer, inner]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert!(wkt.starts_with("POLYGON ((0 0"));
    assert!(wkt.contains("(2 2"));
}

// =========================================================================
// MultiPoint
// =========================================================================

#[test]
fn test_multipoint() {
    let wkb = wkb_multipoint(&[(1.0, 2.0), (3.0, 4.0)]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "MULTIPOINT ((1 2), (3 4))");
}

// =========================================================================
// MultiPolygon
// =========================================================================

#[test]
fn test_multipolygon_single() {
    let ring = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    let wkb = wkb_multipolygon(&[vec![ring]]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert_eq!(wkt, "MULTIPOLYGON (((0 0, 0 1, 1 1, 1 0, 0 0)))");
}

#[test]
fn test_multipolygon_two() {
    let ring1 = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    let ring2 = vec![(2.0, 2.0), (2.0, 3.0), (3.0, 3.0), (3.0, 2.0), (2.0, 2.0)];
    let wkb = wkb_multipolygon(&[vec![ring1], vec![ring2]]);
    let wkt = wkb_to_wkt(&wkb).unwrap();
    assert!(wkt.starts_with("MULTIPOLYGON (((0 0"));
    assert!(wkt.contains("(2 2"));
}

// =========================================================================
// Error cases
// =========================================================================

#[test]
fn test_empty_wkb() {
    let result = wkb_to_wkt(&[]);
    assert!(result.is_err());
}

#[test]
fn test_truncated_wkb() {
    let wkb = vec![1u8, 1, 0, 0]; // too short for Point
    let result = wkb_to_wkt(&wkb);
    assert!(result.is_err());
}

#[test]
fn test_unsupported_geom_type() {
    let mut buf = Vec::new();
    buf.push(1); // LE
    buf.extend_from_slice(&99u32.to_le_bytes()); // invalid type
    let result = wkb_to_wkt(&buf);
    assert!(result.is_err());
}
