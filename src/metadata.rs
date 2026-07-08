//! Metadata and download utilities for geobr data.

const META_URL: &str = "https://api.github.com/repos/ipea/geobr_prep_data/releases/tags/v2.0.0";
const DL_BASE: &str = "https://github.com/ipea/geobr_prep_data/releases/download/v2.0.0";
const MIRROR_BASE: &str = "https://www.ipea.gov.br/geobr/data_v2.0.0";

/// Map of geography name → prefix used in parquet filenames.
/// e.g. "states" → "states_2022_simplified.parquet"
pub fn geography_prefix() -> &'static [(&'static str, &'static str)] {
    &[
        ("country", "country"),
        ("states", "states"),
        ("regions", "regions"),
        ("municipalities", "municipalities"),
        ("municipal_seat", "municipalseats"),
        ("meso_region", "mesoregions"),
        ("micro_region", "microregions"),
        ("intermediate_region", "intermediateregions"),
        ("immediate_region", "immediateregions"),
        ("biomes", "biomes"),
        ("metro_area", "metroarea"),
        ("urban_area", "urbanareas"),
        ("amazonia_legal", "amazonialegal"),
        ("semi_arid", "semiarid"),
        ("census_tract", "censustracts"),
        ("weighting_area", "weightingareas"),
        ("statistical_grid", "statsgrid"),
        ("indigenous_land", "indigenouslands"),
        ("conservation_unit", "conservationunits"),
        ("disaster_risk_area", "disasterriskareas"),
        ("favelas", "favelas"),
        ("quilombola_land", "quilombolalands"),
        ("health_region", "healthregions"),
        ("health_facility", "healthfacilities"),
        ("schools", "schools"),
        ("polling_place", "pollingplaces"),
        ("pop_arrangement", "poparrangements"),
        ("neighborhood", "neighborhoods"),
    ]
}

/// Parsed metadata entry for a dataset.
#[derive(Debug, Clone)]
pub struct GeoMeta {
    pub file_name: String,
    pub geography: String,
    pub year: u32,
    pub simplified: bool,
}

/// Fetch and parse the list of available parquet files from the geobr release.
pub fn fetch_metadata() -> Result<Vec<GeoMeta>, String> {
    let resp = ureq::get(META_URL)
        .set("User-Agent", "haygeobr")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("cannot fetch geobr metadata: {e}"))?;

    let body = resp.into_string().unwrap_or_default();
    let release: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("cannot parse geobr release JSON: {e}"))?;

    let assets = release
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or("no assets in geobr release")?;

    let mut entries = Vec::new();
    for asset in assets {
        if let Some(name) = asset.get("name").and_then(|n| n.as_str()) {
            if name.ends_with(".parquet") {
                if let Some(meta) = parse_filename(name) {
                    entries.push(meta);
                }
            }
        }
    }

    if entries.is_empty() {
        return Err("no parquet files found in geobr release".into());
    }

    Ok(entries)
}

/// Parse a filename like "states_2022_simplified.parquet" into metadata.
fn parse_filename(fname: &str) -> Option<GeoMeta> {
    if !fname.ends_with(".parquet") {
        return None;
    }
    let stem = fname.strip_suffix(".parquet")?;
    let simplified = stem.ends_with("_simplified");
    let stem = if simplified {
        stem.strip_suffix("_simplified")?
    } else {
        stem
    };

    // Extract year (4-digit number)
    let year = extract_year(stem)?;
    let geography = stem.to_string();

    Some(GeoMeta {
        file_name: fname.to_string(),
        geography,
        year,
        simplified,
    })
}

/// Extract the first 4-digit year from a string.
fn extract_year(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len().saturating_sub(3) {
        let slice: String = chars[i..i + 4].iter().collect();
        if let Ok(y) = slice.parse::<u32>() {
            if (1870..=2100).contains(&y) {
                return Some(y);
            }
        }
    }
    None
}

/// Find the best matching parquet file for a geography and year.
pub fn find_file<'a>(
    metadata: &'a [GeoMeta],
    geography: &str,
    year: Option<u32>,
    simplified: bool,
) -> Result<&'a GeoMeta, String> {
    let prefix = geography_prefix()
        .iter()
        .find(|(name, _)| *name == geography)
        .map(|(_, prefix)| *prefix)
        .ok_or_else(|| format!("unknown geography: '{geography}'"))?;

    let candidates: Vec<&GeoMeta> = metadata
        .iter()
        .filter(|m| m.simplified == simplified && m.geography.starts_with(prefix))
        .collect();

    if candidates.is_empty() {
        return Err(format!(
            "no data found for geography '{geography}' (simplified={simplified})"
        ));
    }

    // If year specified, find exact match
    if let Some(y) = year {
        if let Some(m) = candidates.iter().find(|m| m.year == y) {
            return Ok(m);
        }
        // Find closest year
        let closest = candidates
            .iter()
            .min_by_key(|m| (m.year as i64 - y as i64).unsigned_abs())
            .unwrap();
        return Ok(closest);
    }

    // No year specified: use the latest
    let latest = candidates.iter().max_by_key(|m| m.year).unwrap();
    Ok(latest)
}

/// Download a parquet file to a local path. Tries GitHub first, IPEA mirror as fallback.
pub fn download_parquet(file_name: &str, dest: &std::path::Path) -> Result<(), String> {
    // Try GitHub release URL
    let url1 = format!("{DL_BASE}/{file_name}");
    if try_download(&url1, dest).is_ok() {
        return Ok(());
    }

    // Fallback: IPEA mirror
    let url2 = format!("{MIRROR_BASE}/{file_name}");
    try_download(&url2, dest).map_err(|e| format!("download failed for {file_name}: {e}"))?;

    Ok(())
}

fn try_download(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("User-Agent", "haygeobr")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest)
        .map_err(|e| format!("cannot create file {}: {e}", dest.display()))?;
    std::io::copy(&mut reader, &mut file).map_err(|e| format!("write error: {e}"))?;
    Ok(())
}
