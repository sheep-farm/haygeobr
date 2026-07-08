# haygeobr

Official Brazilian spatial data plugin for [Hayashi](https://github.com/sheep-farm/hayashi) — powered by [geobr/IPEA](https://github.com/ipea/geobr).

Downloads official spatial data sets of Brazil (states, municipalities, biomes, regions, and more) as DataFrames with WKT geometry strings.

## Install

```text
hay install sheep-farm/haygeobr
```

## Usage

```text
import("sheep-farm/haygeobr", as=geobr)

// All states (latest year)
let ufs = geobr::read_state({})

// States for a specific year
let ufs22 = geobr::read_state({"year": 2022})

// Filter a specific state in Hayashi
let rj = filter(ufs22, abbrev_state == "RJ")

// Whole country
let br = geobr::read_country({"year": 2024})

// Biomes
let bio = geobr::read_biomes({"year": 2019})

// List available datasets
let ds = geobr::list_datasets()

// List available years for a dataset
let years = geobr::list_years("states")
```

## Available functions

| Function | Description |
|---|---|
| `read_country` | Brazil country boundary |
| `read_state` | Brazilian states (UF) |
| `read_region` | Brazilian regions (N, NE, SE, S, CO) |
| `read_municipality` | Municipalities |
| `read_meso_region` | Meso-regions |
| `read_micro_region` | Micro-regions |
| `read_biomes` | Biomes (Amazônia, Cerrado, Caatinga, Mata Atlântica, Pampa, Pantanal) |
| `read_amazonia_legal` | Legal Amazon area |
| `read_semi_arid` | Semiarid region |
| `read_indigenous_land` | Indigenous lands |
| `read_conservation_unit` | Conservation units |
| `read_metro_area` | Metropolitan areas |
| `list_datasets` | List all available geographies |
| `list_years` | List available years for a geography |

## Options

All `read_*` functions take a single dict argument:

- `year`: year of the dataset (default: latest available)
- `simplified`: use simplified geometry (default: `true` — smaller files, recommended for maps)

## Columns

Each dataset returns a DataFrame with geography-specific columns plus a `geometry` column containing WKT (Well-Known Text) strings.

Example — `read_state`:
| code_state | name_state | abbrev_state | code_region | name_region | year | geometry |
|---|---|---|---|---|---|---|
| 11 | Rondônia | RO | 1 | Norte | 2022 | MULTIPOLYGON (...) |

## Data source

All data is downloaded from the [geobr_prep_data](https://github.com/ipea/geobr_prep_data) GitHub release (v2.0.0). Files are cached in `~/.hay/cache/geobr/` after the first download.

## License

MIT
