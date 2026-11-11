use chrono::{Local, SubsecRound};
use color_eyre::{Result, eyre::eyre};
use geo::{HasDimensions, MultiPolygon};
use metsaval::consts::{CYAN, GREEN, RED, RESET, YELLOW};
use metsaval::consts::{EPSG_TM35FIN, EPSG_WGS84, SMK_STAND_WFS};
use metsaval::flagger;
use metsaval::model::Stand;
use proj::Proj;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    fs::read_to_string,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct Config {
    api_key: String,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = flagger::parse_args(cli_flags(), None)?;

    match parse_options(args) {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::ListMunicipalities => {
            list_municipalities();
            Ok(())
        }
        Command::SetApi(key) => set_api(&key),
        Command::RemoveMunicipality(key) => {
            remove_municipality(&key);
            Ok(())
        }
        Command::Usage => {
            print_usage();
            std::process::exit(2);
        }
        Command::Run(options) => run(options),
    }
}

/// Jäsennetyistä argumenteista valittu toiminto.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    ListMunicipalities,
    SetApi(String),
    RemoveMunicipality(String),
    /// Kiinteistötunnus puuttui tai argumentteja oli liikaa
    Usage,
    Run(RunOptions),
}

/// `run`-toiminnon valinnat argumenttirivistä.
#[derive(Debug, Default, PartialEq, Eq)]
struct RunOptions {
    property_id: String,
    open_browser: bool,
    gpkg_download_dir: PathBuf,
    gpkg_path: PathBuf,
}

/// Ohjelman liput ja niiden arvot.
fn cli_flags() -> flagger::Flagset {
    flagger::Flagset {
        flags: vec!["h", "help", "kunta-lista", "selain"].into(),
        value_flags: vec!["aseta-api", "poista-kunta", "gpkg-kohde", "gpkg-tiedosto"].into(),
        opt_flags: None,
    }
}

/// Muuntaa jäsennetyt argumentit suoritettavaksi toiminnoksi. Erittely
/// erillään `main`-funktiosta, jotta lippujen käsittely on testattavissa
/// ilman `process::exit`-kutsuja.
fn parse_options(args: flagger::Arguments) -> Command {
    let mut options = RunOptions::default();

    if let Some(flags) = args.flags {
        for flag in flags {
            match flag.as_str() {
                "h" | "help" => return Command::Help,
                "kunta-lista" => return Command::ListMunicipalities,
                "selain" => options.open_browser = true,
                _ => {}
            }
        }
    }

    if let Some(value_flags) = args.value_flags {
        for (flag, value) in value_flags {
            match flag.as_str() {
                "aseta-api" => return Command::SetApi(value),
                "poista-kunta" => return Command::RemoveMunicipality(value),
                "gpkg-kohde" => {
                    options.gpkg_download_dir = PathBuf::from(&value);
                    println!("{YELLOW}GeoPackage ladataan hakemistoon {CYAN}{value}{RESET}");
                }
                "gpkg-tiedosto" => {
                    options.gpkg_path = PathBuf::from(&value);
                    println!("{YELLOW}GeoPackage haetaan tiedostosta {CYAN}{value}{RESET}");
                }
                _ => {}
            }
        }
    }

    let Some([raw_property_id]) = args.normal_str.as_deref() else {
        return Command::Usage;
    };

    options.property_id = raw_property_id.to_string();
    Command::Run(options)
}

/// Kiinteistön tietojen haku, laskenta ja tiedostojen luonti.
fn run(options: RunOptions) -> Result<()> {
    let output_file_dir = dirs::download_dir();

    let property_id = &pad_property_id(&options.property_id)?;
    let config = load_config()?;
    let municipality_name = &parse_municipality(property_id)?;

    println!("Kiinteistötunnus: {CYAN}{property_id}{RESET}");
    println!("Kunta: {CYAN}{municipality_name}{RESET}");

    let (property_geometry, prop_feature_count) =
        get_property_geometry(property_id, &config.api_key)?;
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

    let bounds = metsaval::geo_utils::bounds(&property_geometry)
        .ok_or_else(|| color_eyre::eyre::eyre!("Kiinteistön geometria on tyhjä."))?;
    let bbox = format_bbox(bounds);

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
    let mut gpkg_download_dir = options.gpkg_download_dir;
    let mut gpkg_path = options.gpkg_path;
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

    let stand_ids = collect_stand_ids(&stands)?;

    let (strata, stand_summaries) = metsaval::metsakeskus::read_trees(&gpkg_path, &stand_ids)?;

    // Arvon laskenta
    let species_summary = metsaval::valuation::build_species_summary(&strata, &stands)?;
    let timber_values = metsaval::valuation::build_timber_values(&species_summary);
    let stand_values =
        metsaval::valuation::build_stand_values(&species_summary, &stand_summaries, &stands);
    let raw_rows = metsaval::valuation::build_raw_rows(&strata, &stands, &stand_summaries)?;
    let fixed_costs = metsaval::valuation::build_fixed_costs(&stand_values);
    let totals = metsaval::valuation::compute_totals(&stands, &stand_values);

    // Tulostus
    let summary_str =
        metsaval::valuation::create_dataset_summary(&stand_values, &fixed_costs, &totals);
    println!("{summary_str}");

    // Kysy käyttäjältä luodaanko tiedostot
    let stdin = std::io::stdin();
    if !ask_confirmation_from(&mut stdin.lock(), &mut std::io::stdout()) {
        return Ok(());
    }

    // Maps tason luonti
    let kml_file = match &output_file_dir {
        Some(dir) => dir.join(generate_filename(property_id, ".kml")),
        None => PathBuf::from(generate_filename(property_id, ".kml")),
    };
    let kml_data =
        metsaval::maps::generate_kml(&stand_features, &property_geometry, &stand_values)?;
    metsaval::maps::write_kml(&kml_data, &kml_file)?;
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
        &fixed_costs,
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
    if options.open_browser {
        metsaval::maps::open_browser(maps_url);
    } else {
        metsaval::maps::print_maps_url(maps_url);
    }

    Ok(())
}

/// Muodostaa WFS-kyselyn rajauslaatikon.
fn format_bbox(bounds: (f64, f64, f64, f64)) -> String {
    let (min_x, min_y, max_x, max_y) = bounds;
    format!("{min_x},{min_y},{max_x},{max_y},{EPSG_TM35FIN}")
}

/// Kerää kuvioista niistä puuttuvat `standid`-tunnukset kasvavassa
/// järjestyksessä. Virhe jos yhtään tunnusta ei löydy.
fn collect_stand_ids(stands: &[Stand]) -> Result<Vec<i64>> {
    let mut stand_ids: Vec<i64> = stands.iter().filter_map(|stand| stand.standid).collect();
    stand_ids.sort_unstable();
    stand_ids.dedup();

    if stand_ids.is_empty() {
        color_eyre::eyre::bail!("Kuvioilta ei löytynyt standid-tunnuksia.");
    }

    Ok(stand_ids)
}

/// Lisää kiinteistötunnukseen puuttuvat nollat jotta pituus on oikea
fn pad_property_id(id: &str) -> Result<String> {
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 4 {
        return Err(eyre!("Virheellinen kiinteistötunnus"));
    }

    let mut padded_parts = Vec::new();
    // Ensimmäiset 2 on 3 numeroa
    for &item in parts.iter().take(2) {
        let mut value = String::from(item);
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
    for &item in parts.iter().take(4).skip(2) {
        let mut value = String::from(item);
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
    out.push('-');
    out.push_str(&padded_parts[1]);
    out.push('-');
    out.push_str(&padded_parts[2]);
    out.push('-');
    out.push_str(&padded_parts[3]);

    Ok(out)
}

/// Kysyy käyttäjältä halutaanko tehdä xlsx ja kml tiedostot. Syöte luetaan
/// annetusta lukijasta ja kehoite kirjoitetaan annettuun kirjoittajaan.
fn ask_confirmation_from<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> bool {
    let mut s = String::new();
    let _ = write!(
        writer,
        "Haluatko luoda {CYAN}xlsx{RESET} ja {CYAN}kml{RESET} tiedostot? [y/n]: "
    );
    let _ = writer.flush();
    reader.read_line(&mut s).expect("Virheellinen syöte");

    if let Some('\n') = s.chars().next_back() {
        s.pop();
    }
    if let Some('\r') = s.chars().next_back() {
        s.pop();
    }

    match s.as_str() {
        "y" | "Y" => true,
        "n" | "N" => false,
        _ => ask_confirmation_from(reader, writer),
    }
}

/// Palauttaa kiinteistön geometrian yhdistettynä yhteen MultiPolygoniin, ja kiinteistöllä sijaitsevien
/// palstojen määrän
fn get_property_geometry(property_id: &str, api_key: &str) -> Result<(MultiPolygon, usize)> {
    // Kiinteistön palstat MML:stä
    let property_features = metsaval::mml::get_property_features(property_id, api_key)?;
    let property_geometry = combine_property_geometries(property_id, &property_features)?;

    Ok((property_geometry, property_features.len()))
}

/// Yhdistää kiinteistön palstojen geometriat yhdeksi yhteiseksi
/// MultiPolygoniksi ja muuttaa ne ETRS-TM35FIN-koordinaatistoon.
fn combine_property_geometries(
    property_id: &str,
    property_features: &[geojson::Feature],
) -> Result<MultiPolygon> {
    if property_features.is_empty() {
        color_eyre::eyre::bail!("Kiinteistölle {property_id} ei löytynyt palstoja MML:stä.");
    }

    let mut geometries_wgs84 = Vec::new();
    for feature in property_features {
        if let Some(geometry) = metsaval::geo_utils::feature_to_multipolygon(feature)
            && !geometry.is_empty()
        {
            geometries_wgs84.push(geometry);
        }
    }

    if geometries_wgs84.is_empty() {
        color_eyre::eyre::bail!("MML palautti palstat, mutta niillä ei ollut geometriaa.");
    }

    // Yhdistetään kaikki palstat ja muutetaan ETRS-TM35FINiin.
    let to_tm35 = Proj::new_known_crs(EPSG_WGS84, EPSG_TM35FIN, None)?;
    let property_wgs84 = metsaval::geo_utils::union_all(&geometries_wgs84);
    Ok(metsaval::geo_utils::transform(&property_wgs84, &to_tm35))
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
    let Some(config_file) = config_file_path() else {
        return Err(eyre!("Kotihakemiston saaminen epäonnistui"));
    };

    load_config_from(&config_file)
}

/// Lukee asetukset annetusta tiedostosta.
fn load_config_from(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;

    Ok(config)
}

/// Tallentaa annetun avaimen ohjelman config.toml tiedostoon
fn set_api(key: &str) -> Result<()> {
    if let Some(config_file) = config_file_path() {
        write_api_key(&config_file, key)?;
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

/// Kirjoittaa API-avaimen annettuun config-tiedostoon.
fn write_api_key(path: &Path, key: &str) -> Result<()> {
    let api_string = format!("api_key = \"{key}\"");
    fs::write(path, api_string)?;
    Ok(())
}

/// Generoi ohjelman tuottamille tiedostoille nimen
fn generate_filename(property_id: &str, suffix: &str) -> String {
    let mut time = Local::now().trunc_subsecs(0).to_rfc3339().replace(":", "-");

    // Poistaa ajasta kellonajan ja aikavyöhykkeen (esim 'T08-00-00 +0300')
    if let Some(pos) = time.find('T') {
        time.truncate(pos);
    }

    let mut filename = property_id.to_owned();
    filename.push_str("__");
    filename.push_str(&time);
    filename.push_str(suffix);

    filename
}

/// Ottaa kiinteistötunnuksesta koodin ennen ensimmäistä vipua (esim.
/// `740-555-0002-0000` -> `740`).
fn municipality_code(property_id: &str) -> Result<String> {
    let Some(pos) = property_id.find('-') else {
        return Err(eyre!("Virheellinen kiinteistötunnus"));
    };

    Ok(property_id[..pos].to_string())
}

fn parse_municipality(property_id: &str) -> Result<String> {
    let code = municipality_code(property_id)?;

    let path = municipality_file();
    let mut municipalities = load_municipalities_from(path.as_deref());
    let known = municipalities.contains_key(&code);

    let stdin = std::io::stdin();
    let municipality_name = resolve_municipality(
        &mut municipalities,
        &code,
        &mut stdin.lock(),
        &mut std::io::stdout(),
    );

    if !known {
        save_municipalities_to(path.as_deref(), &municipalities);
    }

    Ok(municipality_name)
}

/// Etsii koodin vastaavan kunnan. Jos koodia ei ole tallennettu, kysyy kunnan
/// käyttäjältä ja lisää sen listaan.
fn resolve_municipality(
    municipalities: &mut HashMap<String, String>,
    code: &str,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> String {
    if let Some(name) = municipalities.get(code) {
        return name.clone();
    }

    let new_municip = ask_municipality_from(reader, writer);
    municipalities.insert(code.to_string(), new_municip.clone());
    new_municip
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

/// Hakee tallennetut kunnat annetusta tiedostosta. Puuttuva tai
/// virheellinen tiedosto tulee tyhjäksi listaksi.
fn load_municipalities_from(path: Option<&Path>) -> HashMap<String, String> {
    let Some(json) = path.and_then(|path| read_to_string(path).ok()) else {
        return HashMap::new();
    };

    serde_json::from_str(&json).unwrap_or_default()
}

/// Tallentaa kunnat annettuun tiedostoon. Virheitä ei raportoida.
fn save_municipalities_to(path: Option<&Path>, municipalities: &HashMap<String, String>) {
    let Some(path) = path else {
        return;
    };

    if let Ok(json) = serde_json::to_string_pretty(municipalities) {
        let _ = fs::write(path, json);
    }
}

/// Kysyy käyttäjältä kunnan, joka kyseiseen koodiin liitetään
fn ask_municipality_from<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> String {
    let mut s = String::new();
    let _ = write!(writer, "Syötä kunta: ");
    let _ = writer.flush();
    reader.read_line(&mut s).expect("Virheellinen syöte");

    if let Some('\n') = s.chars().next_back() {
        s.pop();
    }
    if let Some('\r') = s.chars().next_back() {
        s.pop();
    }

    s
}

fn remove_municipality(key: &str) {
    remove_municipality_from(municipality_file().as_deref(), key);
    println!("Kunta {YELLOW}{key}{RESET} poistettu");
}

/// Poistaa koodin tallennetujen kuntien listasta.
fn remove_municipality_from(path: Option<&Path>, key: &str) {
    let mut municipalities = load_municipalities_from(path);
    municipalities.remove(key);
    save_municipalities_to(path, &municipalities);
}

fn list_municipalities() {
    list_municipalities_from(municipality_file().as_deref());
}

/// Tulostaa kaikki tallennetut kunnat ja niiden koodit.
fn list_municipalities_from(path: Option<&Path>) {
    let municipalities = load_municipalities_from(path);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Yksilöllinen polku järjestelmän väliaikaiskansioon testiä varten.
    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("metsaval_main_{}_{}", std::process::id(), name));
        path
    }

    /// Jäsentää argumentit samoin kuin ohjelma käynnistyksessä.
    fn parse(args: &[&str]) -> Command {
        parse_options(flagger::parse_args(cli_flags(), Some(args)).unwrap())
    }

    #[test]
    fn property_id_is_padded_to_full_length() {
        assert_eq!(
            pad_property_id("12-345-6-789").unwrap(),
            "012-345-0006-0789"
        );
        assert_eq!(
            pad_property_id("001-002-0003-0004").unwrap(),
            "001-002-0003-0004"
        );
        assert_eq!(pad_property_id("740-555-2-4").unwrap(), "740-555-0002-0004");
    }

    #[test]
    fn property_id_with_wrong_part_count_is_rejected() {
        assert!(pad_property_id("12345678901234").is_err());
        assert!(pad_property_id("12-345-6").is_err());
        assert!(pad_property_id("").is_err());
        assert!(pad_property_id("12-345-6-789-0").is_err());
    }

    #[test]
    fn property_id_with_too_long_parts_is_rejected() {
        assert!(pad_property_id("1234-345-6-789").is_err());
        assert!(pad_property_id("12-345-67890-789").is_err());
        assert!(pad_property_id("12345-345-6-789").is_err());
    }

    #[test]
    fn property_id_parts_of_exact_length_are_kept_as_they_are() {
        assert_eq!(
            pad_property_id("12-345-6-7890").unwrap(),
            "012-345-0006-7890"
        );
        assert_eq!(pad_property_id("123-4-5-6").unwrap(), "123-004-0005-0006");
    }

    #[test]
    fn filename_contains_property_id_date_and_suffix() {
        let name = generate_filename("12-345-6-789", ".kml");
        assert!(name.starts_with("12-345-6-789__"), "nimi: {name}");
        assert!(name.ends_with(".kml"), "nimi: {name}");

        // Päivämäärä muodossa YYYY-MM-DD ilman kellonaikaa
        let middle = name
            .trim_start_matches("12-345-6-789__")
            .trim_end_matches(".kml");
        assert_eq!(middle.len(), 10, "aikaleima: {middle}");
        assert!(!middle.contains('T'), "aikaleima: {middle}");
        assert!(!middle.contains(':'), "aikaleima: {middle}");
        assert!(middle.starts_with("20"), "aikaleima: {middle}");

        let excel_name = generate_filename("001-002-0003-0004", ".xlsx");
        assert!(excel_name.starts_with("001-002-0003-0004__"));
        assert!(excel_name.ends_with(".xlsx"));
    }

    #[test]
    fn filename_is_stable_within_the_same_day() {
        let first = generate_filename("1-2-3-4", ".kml");
        let second = generate_filename("1-2-3-4", ".kml");
        // Molemmat koostuvat samasta päivämäärämuotoisesta aikaleimasta
        for name in [first, second] {
            let middle = name
                .trim_start_matches("1-2-3-4__")
                .trim_end_matches(".kml");
            assert_eq!(middle.len(), 10, "aikaleima: {middle}");
            assert!(middle.starts_with("20"), "aikaleima: {middle}");
        }
    }

    // -----------------------------------------------------------------------
    // parse_options
    // -----------------------------------------------------------------------

    #[test]
    fn help_flags_select_the_help_command() {
        assert_eq!(parse(&["--help"]), Command::Help);
        assert_eq!(parse(&["-h"]), Command::Help);
        assert_eq!(parse(&["--selain", "--help"]), Command::Help);
    }

    #[test]
    fn municipality_list_flag_selects_the_list_command() {
        assert_eq!(parse(&["--kunta-lista"]), Command::ListMunicipalities);
    }

    #[test]
    fn value_flags_select_their_own_commands() {
        assert_eq!(
            parse(&["--aseta-api", "avain123"]),
            Command::SetApi("avain123".to_string())
        );
        assert_eq!(
            parse(&["--poista-kunta", "740"]),
            Command::RemoveMunicipality("740".to_string())
        );
        // Palvelukset hoituvat ennen kiinteistötunnusta
        assert_eq!(
            parse(&["740-555-0002-0000", "--aseta-api", "avain123"]),
            Command::SetApi("avain123".to_string())
        );
    }

    #[test]
    fn browser_flag_is_recorded_for_the_run() {
        let Command::Run(options) = parse(&["740-555-0002-0000", "--selain"]) else {
            panic!("odotettiin Run-komentoa");
        };
        assert!(options.open_browser);
        assert_eq!(options.property_id, "740-555-0002-0000");
    }

    #[test]
    fn run_defaults_to_printing_the_maps_url() {
        let Command::Run(options) = parse(&["740-555-0002-0000"]) else {
            panic!("odotettiin Run-komentoa");
        };
        assert!(!options.open_browser);
        assert!(options.gpkg_path.is_empty());
        assert!(options.gpkg_download_dir.is_empty());
    }

    #[test]
    fn gpkg_options_are_recorded_for_the_run() {
        let Command::Run(options) = parse(&[
            "--gpkg-kohde",
            "/tmp/kohde",
            "--gpkg-tiedosto",
            "/tmp/data.gpkg",
            "740-555-0002-0000",
        ]) else {
            panic!("odotettiin Run-komentoa");
        };
        assert_eq!(options.gpkg_download_dir, PathBuf::from("/tmp/kohde"));
        assert_eq!(options.gpkg_path, PathBuf::from("/tmp/data.gpkg"));
        assert_eq!(options.property_id, "740-555-0002-0000");
    }

    #[test]
    fn missing_or_ambiguous_property_ids_select_usage() {
        assert_eq!(parse(&[]), Command::Usage);
        assert_eq!(parse(&["--selain"]), Command::Usage);
        assert_eq!(
            parse(&["740-555-0002-0000", "toinen-tunnus"]),
            Command::Usage
        );
    }

    #[test]
    fn help_and_usage_texts_are_printed() {
        print_help();
        print_usage();
    }

    // -----------------------------------------------------------------------
    // Apufunktiot laskennalle ja tiedostojenkäsittelylle
    // -----------------------------------------------------------------------

    #[test]
    fn format_bbox_uses_the_bounds_and_the_tm35fin_crs() {
        assert_eq!(
            format_bbox((385_000.0, 6_700_000.0, 385_100.0, 6_700_100.0)),
            "385000,6700000,385100,6700100,EPSG:3067"
        );
    }

    #[test]
    fn stand_ids_are_sorted_and_deduplicated() {
        let stands = vec![
            Stand {
                standid: Some(3),
                ..Stand::default()
            },
            Stand {
                standid: Some(1),
                ..Stand::default()
            },
            Stand {
                standid: Some(1),
                ..Stand::default()
            },
            Stand::default(),
        ];
        assert_eq!(collect_stand_ids(&stands).unwrap(), vec![1, 3]);
    }

    #[test]
    fn stands_without_stand_ids_are_an_error() {
        assert!(collect_stand_ids(&[Stand::default()]).is_err());
        assert!(collect_stand_ids(&[]).is_err());
    }

    /// Kiinteistön palsta WGS84-koordinaatistossa (n. 110 m × 110 m).
    fn wgs84_parcel(min_lon: f64, min_lat: f64) -> geojson::Feature {
        serde_json::from_value(serde_json::json!({
            "type": "Feature",
            "geometry": {"type": "Polygon", "coordinates": [[
                [min_lon, min_lat],
                [min_lon + 0.001, min_lat],
                [min_lon + 0.001, min_lat + 0.001],
                [min_lon, min_lat + 0.001],
                [min_lon, min_lat]
            ]]},
            "properties": {"kiinteistotunnus": "74055500020000"}
        }))
        .unwrap()
    }

    #[test]
    fn combine_property_geometries_unions_and_projects_the_parcels() {
        let features = vec![wgs84_parcel(24.0, 60.0), wgs84_parcel(24.002, 60.002)];
        let geometry = combine_property_geometries("740-555-0002-0000", &features).unwrap();

        assert!(!geometry.is_empty());
        assert_eq!(
            metsaval::geo_utils::geometry_type(&geometry),
            "MultiPolygon"
        );

        // Koordinaatit on muutettu ETRS-TM35FINiin
        let (min_x, min_y, max_x, max_y) = metsaval::geo_utils::bounds(&geometry).unwrap();
        assert!(
            (100_000.0..=800_000.0).contains(&min_x) && (100_000.0..=800_000.0).contains(&max_x),
            "TM35FIN x-koordinaatit: {min_x}..{max_x}"
        );
        assert!(
            (6_000_000.0..=7_800_000.0).contains(&min_y)
                && (6_000_000.0..=7_800_000.0).contains(&max_y),
            "TM35FIN y-koordinaatit: {min_y}..{max_y}"
        );
    }

    #[test]
    fn combine_property_geometries_rejects_an_empty_feature_list() {
        let err = combine_property_geometries("740-555-0002-0000", &[]).unwrap_err();
        assert!(err.to_string().contains("ei löytynyt palstoja"));
    }

    #[test]
    fn combine_property_geometries_rejects_features_without_geometry() {
        let mut feature = wgs84_parcel(24.0, 60.0);
        feature.geometry = None;

        let err = combine_property_geometries("740-555-0002-0000", &[feature]).unwrap_err();
        assert!(err.to_string().contains("ei ollut geometriaa"));
    }

    #[test]
    fn municipality_code_is_the_part_before_the_first_dash() {
        assert_eq!(municipality_code("740-555-0002-0000").unwrap(), "740");
        assert_eq!(municipality_code("1-2").unwrap(), "1");
        assert!(municipality_code("74055500020000").is_err());
        assert!(municipality_code("").is_err());
    }

    #[test]
    fn resolve_municipality_returns_known_names_without_asking() {
        let mut municipalities = HashMap::from([("740".to_string(), "Testila".to_string())]);
        let mut input: &[u8] = b"";
        let mut prompt = Vec::new();

        let name = resolve_municipality(&mut municipalities, "740", &mut input, &mut prompt);
        assert_eq!(name, "Testila");
        assert!(prompt.is_empty(), "kysymystä ei pitäisi tulla");
    }

    #[test]
    fn resolve_municipality_asks_for_unknown_codes_and_stores_the_answer() {
        let mut municipalities = HashMap::new();
        let mut input: &[u8] = b"Uusikaupunki\n";
        let mut prompt = Vec::new();

        let name = resolve_municipality(&mut municipalities, "577", &mut input, &mut prompt);
        assert_eq!(name, "Uusikaupunki");
        assert_eq!(
            municipalities.get("577").map(String::as_str),
            Some("Uusikaupunki")
        );
        assert!(String::from_utf8_lossy(&prompt).contains("Syötä kunta:"));
    }

    #[test]
    fn municipalities_round_trip_through_a_json_file() {
        let path = temp_path("municipalities.json");
        let municipalities = HashMap::from([
            ("740".to_string(), "Testila".to_string()),
            ("577".to_string(), "Toinen kunta".to_string()),
        ]);

        save_municipalities_to(Some(&path), &municipalities);
        assert_eq!(load_municipalities_from(Some(&path)), municipalities);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_missing_municipality_file_is_an_empty_list() {
        let path = temp_path("ei_ole_municipalities.json");
        let _ = fs::remove_file(&path);

        assert!(load_municipalities_from(Some(&path)).is_empty());
        assert!(load_municipalities_from(None).is_empty());
        // Polkua ei ole, joten tallennus on hiljainen no-op
        save_municipalities_to(None, &HashMap::new());
    }

    #[test]
    fn a_broken_municipality_file_is_an_empty_list() {
        let path = temp_path("broken_municipalities.json");
        fs::write(&path, "tämä ei ole jsonia").unwrap();

        assert!(load_municipalities_from(Some(&path)).is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn removing_a_municipality_rewrites_the_file() {
        let path = temp_path("remove_municipality.json");
        let municipalities = HashMap::from([
            ("740".to_string(), "Testila".to_string()),
            ("577".to_string(), "Toinen kunta".to_string()),
        ]);
        save_municipalities_to(Some(&path), &municipalities);

        remove_municipality_from(Some(&path), "740");

        let loaded = load_municipalities_from(Some(&path));
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get("577").map(String::as_str), Some("Toinen kunta"));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn listing_municipalities_prints_every_entry() {
        let path = temp_path("list_municipalities.json");
        save_municipalities_to(
            Some(&path),
            &HashMap::from([("740".to_string(), "Testila".to_string())]),
        );

        list_municipalities_from(Some(&path));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn config_is_read_from_the_given_file() {
        let path = temp_path("config.toml");
        fs::write(&path, "api_key = \"avain123\"").unwrap();

        let config = load_config_from(&path).unwrap();
        assert_eq!(config.api_key, "avain123");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_missing_or_broken_config_is_an_error() {
        let missing = temp_path("ei_ole_config.toml");
        let _ = fs::remove_file(&missing);
        assert!(load_config_from(&missing).is_err());

        let broken = temp_path("broken_config.toml");
        fs::write(&broken, "avain = {").unwrap();
        assert!(load_config_from(&broken).is_err());

        let _ = fs::remove_file(&broken);
    }

    #[test]
    fn write_api_key_produces_a_readable_config() {
        let path = temp_path("api_key_config.toml");
        write_api_key(&path, "avain123").unwrap();

        assert_eq!(load_config_from(&path).unwrap().api_key, "avain123");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn confirmation_accepts_yes_and_no() {
        let mut prompt = Vec::new();
        assert!(ask_confirmation_from(&mut &b"y\n"[..], &mut prompt));
        assert!(ask_confirmation_from(&mut &b"Y\n"[..], &mut prompt));
        assert!(!ask_confirmation_from(&mut &b"n\n"[..], &mut prompt));
        // Windows-rivinvaihto pureutuu myös
        assert!(!ask_confirmation_from(&mut &b"N\r\n"[..], &mut prompt));

        let text = String::from_utf8_lossy(&prompt).to_string();
        assert!(text.contains("Haluatko luoda"), "kehoite: {text}");
        assert!(text.contains("[y/n]"), "kehoite: {text}");
    }

    #[test]
    fn confirmation_retries_until_a_valid_answer_arrives() {
        let mut prompt = Vec::new();
        assert!(ask_confirmation_from(&mut &b"k\ny\n"[..], &mut prompt));
        assert!(!ask_confirmation_from(&mut &b"x\nn\n"[..], &mut prompt));
    }

    #[test]
    fn municipality_prompt_reads_one_line_and_trims_the_newline() {
        let mut prompt = Vec::new();
        assert_eq!(
            ask_municipality_from(&mut &b"Testila\n"[..], &mut prompt),
            "Testila"
        );
        assert_eq!(
            ask_municipality_from(&mut &b"Toinen\r\n"[..], &mut prompt),
            "Toinen"
        );
        assert_eq!(
            String::from_utf8(prompt).unwrap(),
            "Syötä kunta: Syötä kunta: "
        );
    }
}
