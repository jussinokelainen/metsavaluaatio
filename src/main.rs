mod consts;
mod excel;
mod flagger;
mod geo_utils;
mod http;
mod maps;
mod metsakeskus;
mod mml;
mod model;
mod valuation;
mod wfs;

use crate::consts::{EPSG_TM35FIN, EPSG_WGS84, SMK_STAND_WFS};
use crate::model::Stand;
use chrono::{Local, SubsecRound};
use color_eyre::{Result, eyre::eyre};
use geo::HasDimensions;
use proj::Proj;
use serde::Deserialize;
use std::{collections::HashMap, fs, fs::read_to_string, path::PathBuf};

#[derive(Deserialize)]
struct Config {
    api_key: String,
}

fn config_file_path() -> Option<PathBuf> {
    if let Some(home_dir) = std::env::home_dir() {
        let config_dir = home_dir.join(".config/metsavaluaatio");
        fs::create_dir_all(&config_dir).ok()?;
        Some(config_dir.join("config.toml"))
    } else {
        None
    }
}

/// Hakee ohjelman config.toml tiedoston
fn load_config() -> Result<Config> {
    if let Some(config_file) = config_file_path() {
        let content = fs::read_to_string(config_file)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    } else {
        Err(eyre!("Kotihakemiston saaminen epäonnistui"))
    }
}

/// Tallentaa annetun avaimen ohjelman config.toml tiedostoon
fn set_api(key: String) -> Result<()> {
    if let Some(config_file) = config_file_path() {
        let api_string = format!("api_key = \"{key}\"");
        fs::write(&config_file, api_string)?;
        println!(
            "API avain onnistuneesti tallennettu tiedostoon {}",
            config_file.display()
        );
    } else {
        println!("API avaimen tallentaminen epäonnistui (hakemiston luominen epäonnistui)");
    }
    Ok(())
}

/// Generoi ohjelman tuottamille tiedostoille nimen
fn generate_filename(property_id: &String, suffix: &str) -> String {
    let mut time = Local::now().trunc_subsecs(0).to_rfc3339().replace(":", "-");

    // Poistaa ajasta kellonajan ja aikavyöhykkeen (esim 'T08-00-00 +0300')
    if let Some(pos) = time.find('T') {
        time.truncate(pos);
    }

    let mut filename = property_id.clone();
    filename.push_str("__");
    filename.push_str(&time);
    filename.push_str(suffix);

    filename
}

fn municipality_file() -> Option<PathBuf> {
    if let Some(home_dir) = std::env::home_dir() {
        let local_dir = home_dir.join(".local/share/metsavaluaatio");
        fs::create_dir_all(&local_dir).ok()?;
        Some(local_dir.join("municipalities.json"))
    } else {
        None
    }
}

/// Hakee tallennetut kunnat
fn load_municipalities() -> HashMap<String, String> {
    if let Some(path) = municipality_file() {
        if let Some(json) = read_to_string(path).ok() {
            if let Some(parsed_content) = serde_json::from_str(&json).ok() {
                parsed_content
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    }
}

/// Tallentaa kunnat
fn save_municipalities(municipalities: HashMap<String, String>) {
    if let Some(path) = municipality_file() {
        match serde_json::to_string_pretty(&municipalities) {
            Ok(json) => match fs::write(path, json) {
                Ok(_) => {}
                Err(_) => {}
            },
            Err(_) => {}
        }
    }
}

/// Kysyy käyttäjältä kunnan, joka kyseiseen koodiin liitetään
fn ask_municipality() -> String {
    use std::io::{Write, stdin, stdout};
    let mut s = String::new();
    print!("Syötä kunta: ");
    let _ = stdout().flush();
    stdin().read_line(&mut s).expect("Virheellinen syöte");

    if let Some('\n') = s.chars().next_back() {
        s.pop();
    }
    if let Some('\r') = s.chars().next_back() {
        s.pop();
    }

    s
}

fn remove_municipality(key: String) {
    let mut municipalities = load_municipalities();
    municipalities.remove(&key);
    save_municipalities(municipalities);
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let config = load_config()?;

    let flags = flagger::Flagset {
        flags: Vec::new(),
        value_flags: vec!["aseta-api", "poista-kunta"],
        opt_flags: Vec::new(),
    };
    let args = flagger::parse_args(flags, None)?;
    for (flag, value) in args.valued_flags {
        match flag.as_str() {
            "aseta-api" => {
                set_api(value)?;
                return Ok(());
            }
            "poista-kunta" => {
                remove_municipality(value);
                return Ok(());
            }
            _ => {}
        }
    }

    if args.normal_str.len() != 1 {
        eprintln!("Käyttö: metsaval <KIINTEISTOTUNNUS>");
        std::process::exit(2);
    }

    let property_id = &args.normal_str[0];
    let mml_api_key = config.api_key;

    let mut municipalities = load_municipalities();
    let mut municip_id = property_id.clone();
    let Some(pos) = municip_id.find('-') else {
        return Err(eyre!("Virheellinen kiinteistötunnus"));
    };

    municip_id.truncate(pos);
    let municipality_name = match municipalities.get(&municip_id) {
        Some(name) => name,
        None => {
            // Kysy kunta käyttäjältä ja lisää se tallennettuihin kuntiin
            let new_municip = ask_municipality();
            municipalities.insert(municip_id, new_municip.clone());
            save_municipalities(municipalities);
            &new_municip.clone()
        }
    };

    println!("Kiinteistötunnus: {property_id}");
    println!("Kunta: {municipality_name}");

    let to_tm35 = Proj::new_known_crs(EPSG_WGS84, EPSG_TM35FIN, None)?;

    // Kiinteistön palstat MML:stä
    let property_features = mml::get_property_features(property_id, &mml_api_key)?;
    println!("Kiinteistöltä löytyi {} palstaa.", property_features.len());

    if property_features.is_empty() {
        color_eyre::eyre::bail!("Kiinteistölle {property_id} ei löytynyt palstoja MML:stä.");
    }

    let mut geometries_wgs84 = Vec::new();
    for feature in &property_features {
        if let Some(geometry) = geo_utils::feature_to_multipolygon(feature) {
            if !geometry.is_empty() {
                geometries_wgs84.push(geometry);
            }
        }
    }

    if geometries_wgs84.is_empty() {
        color_eyre::eyre::bail!("MML palautti palstat, mutta niillä ei ollut geometriaa.");
    }

    // Yhdistetään kaikki palstat ja muutetaan ETRS-TM35FINiin.
    let property_wgs84 = geo_utils::union_all(&geometries_wgs84);
    let property_geometry = geo_utils::transform(&property_wgs84, &to_tm35);

    println!(
        "Geometriatyyppi: {}",
        geo_utils::geometry_type(&property_geometry)
    );
    println!(
        "Pinta-ala: {:.3} ha",
        geo_utils::area_m2(&property_geometry) / 10_000.0
    );

    // Metsävarakuviot Metsäkeskuksen WFS:stä
    let stand_layers = wfs::get_layers(SMK_STAND_WFS)?;
    println!("Metsävarakerroksia löytyi: {}", stand_layers.len());

    let stand_layer = wfs::select_stand_layer(&stand_layers)?;
    println!("Käytettävä metsävaralayer on {stand_layer}");

    let (min_x, min_y, max_x, max_y) = geo_utils::bounds(&property_geometry)
        .ok_or_else(|| color_eyre::eyre::eyre!("Kiinteistön geometria on tyhjä."))?;
    let bbox = format!("{min_x},{min_y},{max_x},{max_y},EPSG:3067");

    println!("Kiinteistön BBOX:");
    println!("{bbox}");

    let stand_features =
        wfs::get_features_bbox(SMK_STAND_WFS, &stand_layer, &bbox, EPSG_TM35FIN, 10_000)?;
    println!("BBOX-alueelta löytyi: {} kuviota", stand_features.len());

    let stands: Vec<Stand> = stand_features
        .iter()
        .filter_map(|feature| geo_utils::clip_stand(feature, &property_geometry))
        .collect();

    println!(
        "Kiinteistöllä sijaitsevia/leikkaavia metsäkuvioita: {}",
        stands.len()
    );

    if stands.is_empty() {
        color_eyre::eyre::bail!("Kiinteistöltä ei löytynyt yhtään metsäkuviota.");
    }

    // Puustotiedot Metsäkeskuksen GeoPackagesta
    let gpkg_path = metsakeskus::download_gpkg(municipality_name)?;

    let mut stand_ids: Vec<i64> = stands.iter().filter_map(|stand| stand.standid).collect();
    stand_ids.sort_unstable();
    stand_ids.dedup();

    if stand_ids.is_empty() {
        color_eyre::eyre::bail!("Kuvioilta ei löytynyt standid-tunnuksia.");
    }

    let (strata, stand_summaries) = metsakeskus::read_trees(&gpkg_path, &stand_ids)?;

    // Arvon laskenta
    let species_summary = valuation::build_species_summary(&strata, &stands)?;
    let timber_values = valuation::build_timber_values(&species_summary);
    let stand_values = valuation::build_stand_values(&species_summary, &stand_summaries, &stands);
    let raw_rows = valuation::build_raw_rows(&strata, &stands)?;

    // Tulostus
    valuation::print_dataset_summary(&stands, &species_summary);

    // Maps tason luonti
    let kml_file = generate_filename(property_id, ".kml");
    maps::write_kml(stand_features, &property_geometry, &kml_file, gpkg_path)?;

    // Excel-yhteenveto
    let totals = valuation::compute_totals(&stands, &species_summary);
    let excel_file = generate_filename(property_id, ".xlsx");
    excel::export(
        &excel_file,
        &totals,
        &stand_values,
        &species_summary,
        &timber_values,
        &raw_rows,
    )?;

    let absolute = std::fs::canonicalize(excel_file)?;
    println!("Excel-yhteenveto tallennettu: {}", absolute.display());

    // Avaa selain kartan luonti sivulla
    let maps_url = "https://www.google.com/mymaps";
    maps::open_browser(maps_url);

    Ok(())
}
