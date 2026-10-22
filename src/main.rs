use chrono::{Local, SubsecRound};
use color_eyre::{Result, eyre::eyre};
use dirs;
use geo::{HasDimensions, MultiPolygon};
use metsaval::consts::{CYAN, GREEN, RED, RESET, YELLOW};
use metsaval::consts::{EPSG_TM35FIN, EPSG_WGS84, SMK_STAND_WFS};
use metsaval::flagger;
use metsaval::model::Stand;
use proj::Proj;
use serde::Deserialize;
use std::{collections::HashMap, fs, fs::read_to_string, path::PathBuf};

#[derive(Deserialize)]
struct Config {
    api_key: String,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut open_browser = false;

    let flags = flagger::Flagset {
        flags: vec!["h", "help", "kunta-lista", "selain"].into(),
        value_flags: vec!["aseta-api", "poista-kunta", "gpkg-kohde", "gpkg-tiedosto"].into(),
        opt_flags: None,
    };
    let args = flagger::parse_args(flags, None)?;
    if let Some(flags) = args.flags {
        for flag in flags {
            match flag.as_str() {
                "h" | "help" => {
                    print_help();
                    return Ok(());
                }
                "kunta-lista" => {
                    list_municipalities();
                    return Ok(());
                }
                "selain" => open_browser = true,
                _ => {}
            }
        }
    }

    let mut gpkg_download_dir = PathBuf::new();
    let mut gpkg_path = PathBuf::new();
    if let Some(value_flags) = args.value_flags {
        for (flag, value) in value_flags {
            match flag.as_str() {
                "aseta-api" => {
                    set_api(value)?;
                    return Ok(());
                }
                "poista-kunta" => {
                    remove_municipality(value);
                    return Ok(());
                }
                "gpkg-kohde" => {
                    gpkg_download_dir = PathBuf::from(&value);
                    println!("{YELLOW}GeoPackage ladataan hakemistoon {CYAN}{value}{RESET}");
                }
                "gpkg-tiedosto" => {
                    gpkg_path = PathBuf::from(&value);
                    println!("{YELLOW}GeoPackage haetaan tiedostosta {CYAN}{value}{RESET}");
                }
                _ => {}
            }
        }
    }

    let Some([raw_property_id]) = args.normal_str.as_deref() else {
        print_usage();
        std::process::exit(2);
    };

    let output_file_dir = dirs::download_dir();

    let property_id = &pad_property_id(raw_property_id)?;
    let config = load_config()?;
    let municipality_name = &parse_municipality(property_id)?;

    println!("Kiinteistötunnus: {CYAN}{property_id}{RESET}");
    println!("Kunta: {CYAN}{municipality_name}{RESET}");

    let (property_geometry, prop_feature_count) =
        get_property_geometry(property_id, config.api_key)?;
    println!("Kiinteistöltä löytyi {prop_feature_count} palstaa.");

    println!(
        "Geometriatyyppi: {}",
        metsaval::geo_utils::geometry_type(&property_geometry)
    );
    println!(
        "Pinta-ala: {CYAN}{:.3} ha{RESET}",
        metsaval::geo_utils::area_m2(&property_geometry) / 10_000.0
    );

    // Metsävarakuviot Metsäkeskuksen WFS:stä
    let stand_layers = metsaval::wfs::get_layers(SMK_STAND_WFS)?;
    println!("Metsävarakerroksia löytyi: {}", stand_layers.len());

    let stand_layer = metsaval::wfs::select_stand_layer(&stand_layers)?;
    println!("Käytettävä metsävaralayer on {stand_layer}");

    let (min_x, min_y, max_x, max_y) = metsaval::geo_utils::bounds(&property_geometry)
        .ok_or_else(|| color_eyre::eyre::eyre!("Kiinteistön geometria on tyhjä."))?;
    let bbox = format!("{min_x},{min_y},{max_x},{max_y},EPSG:3067");

    println!("Kiinteistön BBOX:");
    println!("{bbox}");

    let stand_features =
        metsaval::wfs::get_features_bbox(SMK_STAND_WFS, &stand_layer, &bbox, EPSG_TM35FIN, 10_000)?;
    println!("BBOX-alueelta löytyi: {} kuviota", stand_features.len());

    let stands: Vec<Stand> = stand_features
        .iter()
        .filter_map(|feature| metsaval::geo_utils::clip_stand(feature, &property_geometry))
        .collect();

    println!(
        "Kiinteistöllä sijaitsevia/leikkaavia metsäkuvioita: {}",
        stands.len()
    );

    if stands.is_empty() {
        color_eyre::eyre::bail!("Kiinteistöltä ei löytynyt yhtään metsäkuviota.");
    }

    // Puustotiedot Metsäkeskuksen GeoPackagesta
    if gpkg_path.is_empty() {
        let gpkg_start_t = Local::now().timestamp_millis();
        println!("Haetaan GeoPackage kunnalle {municipality_name}...");
        if gpkg_download_dir.is_empty() {
            gpkg_download_dir =
                std::env::temp_dir().join(format!("metsavaluaatio_{}", std::process::id()));
        }
        gpkg_path = metsaval::metsakeskus::download_gpkg(municipality_name, gpkg_download_dir)?;
        let gpkg_total_t = (Local::now().timestamp_millis() - gpkg_start_t) as f64 / 1000.0;
        println!(
            "{GREEN}GeoPackage valmis{RESET} ({gpkg_total_t:.1} s): {}",
            gpkg_path.display()
        );
    }

    let mut stand_ids: Vec<i64> = stands.iter().filter_map(|stand| stand.standid).collect();
    stand_ids.sort_unstable();
    stand_ids.dedup();

    if stand_ids.is_empty() {
        color_eyre::eyre::bail!("Kuvioilta ei löytynyt standid-tunnuksia.");
    }

    let (strata, stand_summaries) = metsaval::metsakeskus::read_trees(&gpkg_path, &stand_ids)?;

    // Arvon laskenta
    let species_summary = metsaval::valuation::build_species_summary(&strata, &stands)?;
    let timber_values = metsaval::valuation::build_timber_values(&species_summary);
    let stand_values =
        metsaval::valuation::build_stand_values(&species_summary, &stand_summaries, &stands);
    let raw_rows = metsaval::valuation::build_raw_rows(&strata, &stands, &stand_summaries)?;

    // Tulostus
    metsaval::valuation::print_dataset_summary(&stands, &species_summary);

    // Kysy käyttäjältä luodaanko tiedostot
    if !ask_file_creation() {
        return Ok(());
    }

    // Maps tason luonti
    let kml_file = match &output_file_dir {
        Some(dir) => dir.join(generate_filename(property_id, ".kml")),
        None => PathBuf::from(generate_filename(property_id, ".kml")),
    };
    metsaval::maps::write_kml(stand_features, &property_geometry, &kml_file, &stand_values)?;
    // Yritä näyttää tiedoston täysi polku, näyttää vain tiedoston nimen
    // jos se ei onnistu
    match std::fs::canonicalize(&kml_file) {
        Ok(path) => {
            println!(
                "Google maps taso tallennettu: {CYAN}{}{RESET}",
                path.display()
            );
        }
        Err(_) => println!(
            "Google maps taso tallennettu: {CYAN}{}{RESET}",
            kml_file.display()
        ),
    }

    // Excel-yhteenveto
    let totals = metsaval::valuation::compute_totals(&stands, &species_summary);
    let excel_file = match &output_file_dir {
        Some(dir) => dir.join(generate_filename(property_id, ".xlsx")),
        None => PathBuf::from(generate_filename(property_id, ".xlsx")),
    };
    metsaval::excel::export(
        &excel_file,
        &totals,
        &stand_values,
        &species_summary,
        &timber_values,
        &raw_rows,
    )?;

    // Yritä näyttää tiedoston täysi polku, näyttää vain tiedoston nimen
    // jos se ei onnistu
    match std::fs::canonicalize(&excel_file) {
        Ok(path) => {
            println!(
                "Excel-yhteenveto tallennettu: {CYAN}{}{RESET}",
                path.display()
            );
        }
        Err(_) => println!(
            "Excel-yhteenveto tallennettu: {CYAN}{}{RESET}",
            excel_file.display()
        ),
    }

    // Avaa selain kartan luonti sivulla
    let maps_url = "https://www.google.com/mymaps";
    if open_browser {
        metsaval::maps::open_browser(maps_url);
    } else {
        metsaval::maps::print_maps_url(maps_url);
    }

    Ok(())
}

/// Lisää kiinteistötunnukseen puuttuvat nollat jotta pituus on oikea
fn pad_property_id(id: &String) -> Result<String> {
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 4 {
        return Err(eyre!("Virheellinen kiinteistötunnus"));
    }

    let mut padded_parts = Vec::new();
    // Ensimmäiset 2 on 3 numeroa
    for i in 0..2 {
        let mut value = String::from(parts[i]);
        if value.len() > 3 {
            return Err(eyre!("Virheellinen kiinteistötunnus"));
        }
        if value.len() == 3 {
            padded_parts.push(value);
            continue;
        }
        // Tässä kohtaa pituus on alle 3, pitää lisätä nollat
        let zero_count = 3 - value.len();
        for _ in 0..zero_count {
            let mut tmp = String::from("0");
            tmp.push_str(&value);
            value = tmp;
        }
        padded_parts.push(value);
    }

    // Viimeiset 2 on 4 numeroa
    for i in 2..4 {
        let mut value = String::from(parts[i]);
        if value.len() > 4 {
            return Err(eyre!("Virheellinen kiinteistötunnus"));
        }
        if value.len() == 4 {
            padded_parts.push(value);
            continue;
        }
        // Tässä kohtaa pituus on alle 4, pitää lisätä nollat
        let zero_count = 4 - value.len();
        for _ in 0..zero_count {
            let mut tmp = String::from("0");
            tmp.push_str(&value);
            value = tmp;
        }
        padded_parts.push(value);
    }
    if padded_parts.len() != 4 {
        println!("{padded_parts:?}");
        return Err(eyre!("Virhe jäsentäessä kiinteistötunnusta"));
    }

    let mut out = padded_parts[0].clone();
    out.push_str("-");
    out.push_str(&padded_parts[1]);
    out.push_str("-");
    out.push_str(&padded_parts[2]);
    out.push_str("-");
    out.push_str(&padded_parts[3]);

    Ok(out)
}

/// Kysyy käyttäjältä halutaanko tehdä xlsx ja kml tiedostot
fn ask_file_creation() -> bool {
    use std::io::{Write, stdin, stdout};
    let mut s = String::new();
    print!("Haluatko luoda {CYAN}xlsx{RESET} ja {CYAN}kml{RESET} tiedostot? [y/n]: ");
    let _ = stdout().flush();
    stdin().read_line(&mut s).expect("Virheellinen syöte");

    if let Some('\n') = s.chars().next_back() {
        s.pop();
    }
    if let Some('\r') = s.chars().next_back() {
        s.pop();
    }

    match s.as_str() {
        "y" | "Y" => true,
        "n" | "N" => false,
        _ => ask_file_creation(),
    }
}

/// Palauttaa kiinteistön geometrian yhdistettynä yhteen MultiPolygoniin, ja kiinteistöllä sijaitsevien
/// palstojen määrän
fn get_property_geometry(property_id: &String, api_key: String) -> Result<(MultiPolygon, usize)> {
    // Kiinteistön palstat MML:stä
    let property_features = metsaval::mml::get_property_features(property_id, &api_key)?;

    if property_features.is_empty() {
        color_eyre::eyre::bail!("Kiinteistölle {property_id} ei löytynyt palstoja MML:stä.");
    }

    let mut geometries_wgs84 = Vec::new();
    for feature in &property_features {
        if let Some(geometry) = metsaval::geo_utils::feature_to_multipolygon(feature) {
            if !geometry.is_empty() {
                geometries_wgs84.push(geometry);
            }
        }
    }

    if geometries_wgs84.is_empty() {
        color_eyre::eyre::bail!("MML palautti palstat, mutta niillä ei ollut geometriaa.");
    }

    // Yhdistetään kaikki palstat ja muutetaan ETRS-TM35FINiin.
    let to_tm35 = Proj::new_known_crs(EPSG_WGS84, EPSG_TM35FIN, None)?;
    let property_wgs84 = metsaval::geo_utils::union_all(&geometries_wgs84);
    let property_geometry = metsaval::geo_utils::transform(&property_wgs84, &to_tm35);

    Ok((property_geometry, property_features.len()))
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
            "{GREEN}API avain onnistuneesti tallennettu{RESET} tiedostoon {}",
            config_file.display()
        );
    } else {
        println!(
            "{RED}API avaimen tallentaminen epäonnistui{RESET} (hakemiston luominen epäonnistui)"
        );
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

fn parse_municipality(property_id: &String) -> Result<String> {
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
    Ok(municipality_name.clone())
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
    println!("Kunta {YELLOW}{key}{RESET} poistettu");
}

fn list_municipalities() {
    let municipalities = load_municipalities();
    for municipality in municipalities {
        println!("{}: {}", municipality.0, municipality.1);
    }
}

fn print_usage() {
    eprintln!(
        "Käyttö: metsaval <KIINTEISTÖTUNNUS>
    Lisätietoja: metsaval --help"
    );
}

fn print_help() {
    println!(
        "METSÄVALUAATIO
    Käyttö: metsaval <KIINTEISTÖTUNNUS> [-h | --help] [--kunta-lista] [--aseta-api <api>] [--poista-kunta <koodi>]

        --help, -h      | Näytä tämä viesti
        --kunta-lista   | listaa kaikki tallennetut kunnat ja niiden 3-numeroiset koodit
        --aseta-api     | asettaa MML api-avaimen ohjelman config.toml tiedostoon
        --poista-kunta  | poistaa kunnan ja sen 3-numeroisen koodin tallennettujen kuntien
                        | listasta. Argumenttina vaatii 3-numeroisen koodin, ei kunnan nimeä
        --gpkg-kohde    | Määrittää hakemiston johon GeoPackage ladataan, jos ei halua käyttää
                        | vakio sijaintia (/tmp hakemistoa).
        --gpkg-tiedosto | Määrittää tiedoston josta GeoPackage haetaan. Tämä voi olla hyödyllinen,
                        | jos haluaa hakea tiedot useammasta kiinteistöstä samassa kunnassa, niin
                        | ei tarvitse joka kerta ladata GeoPackagea uudestaan. Ohjelma tulostaa
                        | aina käytetyn GeoPackagen sijainnin, sitä voi käyttää uudestaan jos
                        | tietää kunnan olevan oikea.
        --selain        | Avaa selaimen automaattisesti mymaps sivulla

    Ohjelmalle annetaan argumenttina halutun kiinteistön kiinteistönumero. Jos samassa
    kunnassa ei ole aikaisemmin haettu kiinteistön tietoja, ohjelma kysyy kunnan nimen
    ja tallentaa sen."
    );
}
