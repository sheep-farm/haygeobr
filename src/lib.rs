//! # haygeobr — Official Brazilian spatial data for Hayashi
//!
//! Native plugin that downloads and serves official spatial data sets of
//! Brazil from the [geobr](https://github.com/ipea/geobr) project (IPEA).
//!
//! All data is downloaded from the geobr_prep_data GitHub release as Parquet
//! files, read with Arrow, and returned to Hayashi as DataFrames with WKT
//! geometry strings.
//!
//! ## Usage
//!
//! ```text
//! import("sheep-farm/haygeobr", as=geobr)
//!
//! // All states, latest year
//! let ufs = geobr::read_state({"code": "all"})
//!
//! // Specific state, specific year
//! let rj = geobr::read_state({"code": "RJ", "year": 2022})
//!
//! // Whole country
//! let br = geobr::read_country({"year": 2024})
//!
//! // List available datasets
//! let ds = geobr::list_datasets()
//! ```

#![allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]

use hayashi_plugin_sdk::value::{HayashiValue, IntoHayashi};
use hayashi_plugin_sdk::{hayashi_fn, hayashi_plugin};
use std::sync::Arc;

hayashi_plugin!();

#[cfg(test)]
mod tests {
    use super::metadata::{find_file, geography_prefix, GeoMeta};
    use super::wkb::wkb_to_wkt;

    // ── geography_prefix ──────────────────────────────────────────────────────

    #[test]
    fn test_geography_prefix_not_empty() {
        let prefixes = geography_prefix();
        assert!(!prefixes.is_empty());
    }

    #[test]
    fn test_geography_prefix_known_entries() {
        let prefixes = geography_prefix();
        let find = |name: &str| prefixes.iter().find(|(n, _)| *n == name).map(|(_, p)| *p);
        assert_eq!(find("country"),        Some("country"));
        assert_eq!(find("states"),         Some("states"));
        assert_eq!(find("municipalities"), Some("municipalities"));
        assert_eq!(find("biomes"),         Some("biomes"));
    }

    #[test]
    fn test_geography_prefix_no_duplicates() {
        let prefixes = geography_prefix();
        let mut names: Vec<&str> = prefixes.iter().map(|(n, _)| *n).collect();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "nomes duplicados em geography_prefix");
    }

    // ── find_file (sem rede — usa metadados sintéticos) ───────────────────────

    fn mock_meta() -> Vec<GeoMeta> {
        vec![
            GeoMeta { file_name: "states_2020_simplified.parquet".into(), geography: "states_2020".into(), year: 2020, simplified: true },
            GeoMeta { file_name: "states_2022_simplified.parquet".into(), geography: "states_2022".into(), year: 2022, simplified: true },
            GeoMeta { file_name: "states_2022.parquet".into(),            geography: "states_2022".into(), year: 2022, simplified: false },
            GeoMeta { file_name: "country_2024_simplified.parquet".into(), geography: "country_2024".into(), year: 2024, simplified: true },
        ]
    }

    #[test]
    fn test_find_file_latest() {
        let meta = mock_meta();
        let result = find_file(&meta, "states", None, true).unwrap();
        assert_eq!(result.year, 2022);
    }

    #[test]
    fn test_find_file_exact_year() {
        let meta = mock_meta();
        let result = find_file(&meta, "states", Some(2020), true).unwrap();
        assert_eq!(result.year, 2020);
    }

    #[test]
    fn test_find_file_closest_year() {
        let meta = mock_meta();
        // Pede 2019, que não existe — o mais próximo é 2020
        let result = find_file(&meta, "states", Some(2019), true).unwrap();
        assert_eq!(result.year, 2020);
        // Pede 2023, que não existe — o mais próximo é 2022
        let result2 = find_file(&meta, "states", Some(2023), true).unwrap();
        assert_eq!(result2.year, 2022);
    }

    #[test]
    fn test_find_file_simplified_flag() {
        let meta = mock_meta();
        let simplified = find_file(&meta, "states", Some(2022), true).unwrap();
        assert!(simplified.simplified);
        let detailed = find_file(&meta, "states", Some(2022), false).unwrap();
        assert!(!detailed.simplified);
    }

    #[test]
    fn test_find_file_unknown_geography() {
        let meta = mock_meta();
        assert!(find_file(&meta, "nonexistent_place", None, true).is_err());
    }

}

pub mod metadata;
mod reader;
pub mod wkb;

use metadata::{download_parquet, fetch_metadata, find_file, geography_prefix};
use reader::read_parquet_to_struct;

/// Cache directory for downloaded parquet files.
fn cache_dir() -> std::path::PathBuf {
    let dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let p = std::path::PathBuf::from(dir)
        .join(".hay")
        .join("cache")
        .join("geobr");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Extract options from a HayashiValue dict.
struct GeoOpts {
    year: Option<u32>,
    simplified: bool,
}

impl GeoOpts {
    fn from_value(val: &HayashiValue) -> Self {
        let map = match val {
            HayashiValue::Dict(d) => d,
            _ => return Self::default(),
        };

        let year = match map.get("year") {
            Some(HayashiValue::Int(i)) => Some(*i as u32),
            Some(HayashiValue::Float(f)) => Some(*f as u32),
            _ => None,
        };

        let simplified = match map.get("simplified") {
            Some(HayashiValue::Bool(b)) => *b,
            Some(HayashiValue::Int(i)) => *i != 0,
            _ => true,
        };

        Self { year, simplified }
    }
}

impl Default for GeoOpts {
    fn default() -> Self {
        Self {
            year: None,
            simplified: true,
        }
    }
}

/// Download (or use cached) parquet for a geography, return the local path.
fn get_parquet(
    geography: &str,
    year: Option<u32>,
    simplified: bool,
) -> Result<std::path::PathBuf, String> {
    let meta = fetch_metadata()?;
    let entry = find_file(&meta, geography, year, simplified)?;

    let cache = cache_dir();
    let local_path = cache.join(&entry.file_name);

    if local_path.exists() && local_path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(local_path);
    }

    println!("haygeobr: downloading {}...", entry.file_name);
    download_parquet(&entry.file_name, &local_path)?;
    println!(
        "haygeobr: downloaded {} ({} KB)",
        entry.file_name,
        local_path.metadata().map(|m| m.len() / 1024).unwrap_or(0)
    );

    Ok(local_path)
}

/// Read a parquet file and return as HayashiValue (Arrow StructArray → DataFrame).
fn read_geography(
    geography: &str,
    year: Option<u32>,
    simplified: bool,
    filter_col: Option<&str>,
    filter_val: Option<&str>,
) -> HayashiValue {
    match get_parquet(geography, year, simplified) {
        Ok(path) => match read_parquet_to_struct(&path, filter_col, filter_val) {
            Ok(struct_array) => {
                let array_ref: hayashi_plugin_sdk::arrow::array::ArrayRef = Arc::new(struct_array);
                array_ref.into_hayashi()
            }
            Err(e) => HayashiValue::Str(format!("haygeobr error: {e}")),
        },
        Err(e) => HayashiValue::Str(format!("haygeobr error: {e}")),
    }
}

// =========================================================================
// Public API — all functions take a single dict argument
// =========================================================================

/// haygeobr::read_country({"year": 2024, "simplified": true})
/// Download official spatial data of Brazil (country boundary).
#[hayashi_fn]
pub fn read_country(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("country", o.year, o.simplified, None, None)
}

/// haygeobr::read_state({"code": "all", "year": 2022})
/// code: "all" returns all states. For a specific state, use filter() in Hayashi:
///   let ufs = geobr::read_state({"year": 2022})
///   let rj = filter(ufs, abbrev_state == "RJ")
#[hayashi_fn]
pub fn read_state(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("states", o.year, o.simplified, None, None)
}

/// haygeobr::read_region({"year": 2024})
/// Download official spatial data of Brazilian regions (N, NE, SE, S, CO).
#[hayashi_fn]
pub fn read_region(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("regions", o.year, o.simplified, None, None)
}

/// haygeobr::read_municipality({"code": "all", "year": 2022})
/// Returns all municipalities. Filter in Hayashi for specific state/muni.
#[hayashi_fn]
pub fn read_municipality(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("municipalities", o.year, o.simplified, None, None)
}

/// haygeobr::read_biomes({"year": 2025})
/// Download official spatial data of Brazilian biomes.
#[hayashi_fn]
pub fn read_biomes(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("biomes", o.year, o.simplified, None, None)
}

/// haygeobr::read_meso_region({"code": "all", "year": 2022})
/// Returns all meso-regions. Filter in Hayashi for specific state/region.
#[hayashi_fn]
pub fn read_meso_region(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("meso_region", o.year, o.simplified, None, None)
}

/// haygeobr::read_micro_region({"code": "all", "year": 2022})
/// Returns all micro-regions. Filter in Hayashi for specific state/region.
#[hayashi_fn]
pub fn read_micro_region(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("micro_region", o.year, o.simplified, None, None)
}

/// haygeobr::read_amazonia_legal({"year": 2022})
/// Download spatial data of the Legal Amazon area.
#[hayashi_fn]
pub fn read_amazonia_legal(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("amazonia_legal", o.year, o.simplified, None, None)
}

/// haygeobr::read_semi_arid({"year": 2022})
/// Download spatial data of the Brazilian Semiarid region.
#[hayashi_fn]
pub fn read_semi_arid(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("semi_arid", o.year, o.simplified, None, None)
}

/// haygeobr::read_indigenous_land({"year": 2022})
/// Download spatial data of indigenous lands.
#[hayashi_fn]
pub fn read_indigenous_land(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("indigenous_land", o.year, o.simplified, None, None)
}

/// haygeobr::read_conservation_unit({"year": 202503})
/// Download spatial data of conservation units.
#[hayashi_fn]
pub fn read_conservation_unit(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("conservation_unit", o.year, o.simplified, None, None)
}

/// haygeobr::read_metro_area({"year": 2022})
/// Download spatial data of metropolitan areas.
#[hayashi_fn]
pub fn read_metro_area(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("metro_area", o.year, o.simplified, None, None)
}

/// haygeobr::read_census_tract({"year": 2022})
/// Download spatial data of census tracts (setores censitários).
#[hayashi_fn]
pub fn read_census_tract(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("census_tract", o.year, o.simplified, None, None)
}

/// haygeobr::read_disaster_risk_area({"year": 2022})
/// Download spatial data of disaster risk areas.
#[hayashi_fn]
pub fn read_disaster_risk_area(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("disaster_risk_area", o.year, o.simplified, None, None)
}

/// haygeobr::read_favelas({"year": 2024})
/// Download spatial data of favelas and urban communities.
#[hayashi_fn]
pub fn read_favelas(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("favelas", o.year, o.simplified, None, None)
}

/// haygeobr::read_health_facilities({"year": 2024})
/// Download spatial data of health facilities.
#[hayashi_fn]
pub fn read_health_facilities(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("health_facility", o.year, o.simplified, None, None)
}

/// haygeobr::read_health_region({"year": 2022})
/// Download spatial data of health regions.
#[hayashi_fn]
pub fn read_health_region(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("health_region", o.year, o.simplified, None, None)
}

/// haygeobr::read_immediate_region({"year": 2022})
/// Download spatial data of immediate regions (regiões imediatas).
#[hayashi_fn]
pub fn read_immediate_region(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("immediate_region", o.year, o.simplified, None, None)
}

/// haygeobr::read_intermediate_region({"year": 2022})
/// Download spatial data of intermediate regions (regiões intermediárias).
#[hayashi_fn]
pub fn read_intermediate_region(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("intermediate_region", o.year, o.simplified, None, None)
}

/// haygeobr::read_municipal_seat({"year": 2022})
/// Download spatial data of municipal seats (sede municipal).
#[hayashi_fn]
pub fn read_municipal_seat(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("municipal_seat", o.year, o.simplified, None, None)
}

/// haygeobr::read_neighborhood({"year": 2022})
/// Download spatial data of neighborhoods (bairros).
#[hayashi_fn]
pub fn read_neighborhood(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("neighborhood", o.year, o.simplified, None, None)
}

/// haygeobr::read_polling_place({"year": 2024})
/// Download spatial data of polling places (locais de votação).
#[hayashi_fn]
pub fn read_polling_place(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("polling_place", o.year, o.simplified, None, None)
}

/// haygeobr::read_pop_arrangement({"year": 2022})
/// Download spatial data of population arrangements.
#[hayashi_fn]
pub fn read_pop_arrangement(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("pop_arrangement", o.year, o.simplified, None, None)
}

/// haygeobr::read_quilombola_land({"year": 2024})
/// Download spatial data of quilombola lands (territórios quilombolas).
#[hayashi_fn]
pub fn read_quilombola_land(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("quilombola_land", o.year, o.simplified, None, None)
}

/// haygeobr::read_schools({"year": 2024})
/// Download spatial data of schools.
#[hayashi_fn]
pub fn read_schools(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("schools", o.year, o.simplified, None, None)
}

/// haygeobr::read_statistical_grid({"year": 2022})
/// Download spatial data of statistical grids (grade estatística).
#[hayashi_fn]
pub fn read_statistical_grid(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("statistical_grid", o.year, o.simplified, None, None)
}

/// haygeobr::read_urban_area({"year": 2015})
/// Download spatial data of urban areas (áreas urbanizadas).
#[hayashi_fn]
pub fn read_urban_area(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("urban_area", o.year, o.simplified, None, None)
}

/// haygeobr::read_weighting_area({"year": 2022})
/// Download spatial data of weighting areas (áreas de ponderação).
#[hayashi_fn]
pub fn read_weighting_area(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("weighting_area", o.year, o.simplified, None, None)
}

/// haygeobr::read_capitals()
/// Returns the municipal seats (sede dos municipios) of state capitals.
/// This function downloads the municipalities data.
/// Filter for capitals in Hayashi using multiple filter() and append() calls.
#[hayashi_fn]
pub fn read_capitals(opts: HayashiValue) -> HayashiValue {
    let o = GeoOpts::from_value(&opts);
    read_geography("municipalities", o.year, o.simplified, None, None)
}

/// haygeobr::list_datasets()
/// Returns a list of available geography names.
#[hayashi_fn]
pub fn list_datasets() -> HayashiValue {
    let names: Vec<HayashiValue> = geography_prefix()
        .iter()
        .map(|(name, _)| HayashiValue::Str(name.to_string()))
        .collect();
    HayashiValue::List(names)
}

/// haygeobr::list_years("states")
/// Returns a list of available years for a given geography.
#[hayashi_fn]
pub fn list_years(geography: HayashiValue) -> HayashiValue {
    let geo = match &geography {
        HayashiValue::Str(s) => s.clone(),
        _ => return HayashiValue::Str("haygeobr: expected geography name string".into()),
    };

    match fetch_metadata() {
        Ok(meta) => {
            let prefix = geography_prefix()
                .iter()
                .find(|(name, _)| *name == geo.as_str())
                .map(|(_, p)| *p)
                .unwrap_or(&geo);

            let mut years: Vec<u32> = meta
                .iter()
                .filter(|m| m.simplified && m.geography.starts_with(prefix))
                .map(|m| m.year)
                .collect();
            years.sort();
            years.dedup();

            let vals: Vec<HayashiValue> = years
                .into_iter()
                .map(|y| HayashiValue::Int(y as i64))
                .collect();
            HayashiValue::List(vals)
        }
        Err(e) => HayashiValue::Str(format!("haygeobr error: {e}")),
    }
}
