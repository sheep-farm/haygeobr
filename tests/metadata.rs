//! Unit tests for metadata filename parsing and geography lookup.

use haygeobr::metadata::{geography_prefix, GeoMeta};

// We can't access parse_filename directly (it's private), but we can test
// the public API: geography_prefix and the GeoMeta struct.

#[test]
fn test_geography_prefix_contains_states() {
    let prefixes = geography_prefix();
    assert!(prefixes.iter().any(|(name, _)| *name == "states"));
}

#[test]
fn test_geography_prefix_contains_country() {
    let prefixes = geography_prefix();
    assert!(prefixes.iter().any(|(name, _)| *name == "country"));
}

#[test]
fn test_geography_prefix_contains_municipalities() {
    let prefixes = geography_prefix();
    assert!(prefixes.iter().any(|(name, _)| *name == "municipalities"));
}

#[test]
fn test_geography_prefix_contains_biomes() {
    let prefixes = geography_prefix();
    assert!(prefixes.iter().any(|(name, _)| *name == "biomes"));
}

#[test]
fn test_geography_prefix_maps_correctly() {
    let prefixes = geography_prefix();
    // "states" should map to "states" prefix
    let states = prefixes.iter().find(|(name, _)| *name == "states").unwrap();
    assert_eq!(states.1, "states");
    // "municipalities" should map to "municipalities"
    let munis = prefixes
        .iter()
        .find(|(name, _)| *name == "municipalities")
        .unwrap();
    assert_eq!(munis.1, "municipalities");
    // "municipal_seat" should map to "municipalseats"
    let seat = prefixes
        .iter()
        .find(|(name, _)| *name == "municipal_seat")
        .unwrap();
    assert_eq!(seat.1, "municipalseats");
}

#[test]
fn test_geography_prefix_count() {
    let prefixes = geography_prefix();
    // Should have at least 20 geographies
    assert!(prefixes.len() >= 20);
}

#[test]
fn test_geome_struct_fields() {
    let meta = GeoMeta {
        file_name: "states_2022_simplified.parquet".to_string(),
        geography: "states_2022".to_string(),
        year: 2022,
        simplified: true,
    };
    assert_eq!(meta.file_name, "states_2022_simplified.parquet");
    assert_eq!(meta.year, 2022);
    assert!(meta.simplified);
}
