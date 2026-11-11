//! Päästä päähän -testit oikealle `metsaval`-binäärille.
//!
//! Testit käynnistävät paikallisen valepalvelimen ja ohjaavat ohjelman
//! verkkoliikenteen siihen ympäristömuuttujilla (`METSAVAL_MML_BASE_URL`,
//! `METSAVAL_WFS_URL`, `METSAVAL_GPKG_BASE_URL`). Kotihakemisto ja
//! lataushakemisto ovat väliaikaishakemistoja, joten testit eivät koske
//! käyttäjän oikeisiin tiedostoihin.

use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

// ---------------------------------------------------------------------------
// Väliaikaishakemisto
// ---------------------------------------------------------------------------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "metsaval_cli_{}_{}_{}",
            label,
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Valepalvelin
// ---------------------------------------------------------------------------

/// Reitityssääntö: (avain, sisältötyyppi, runko). Avain täsmää, jos pyynnön
/// kohde sisältää sen.
type Route = (&'static str, &'static str, Vec<u8>);

/// Käynnistää paikallisen HTTP-palvelimen, joka vastaa annetuilla reiteillä.
/// Jokainen yhteys käsitellään omassa säikeessään, jotta rinnakkaiset pyynnöt
/// (esim. GeoPackage ja MML samaan aikaan) toimivat.
fn serve(routes: Vec<Route>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let routes = Arc::new(routes);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let routes = Arc::clone(&routes);
            thread::spawn(move || {
                let request = read_request(&mut stream);
                let target = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");

                let (content_type, body) = routes
                    .iter()
                    .find(|(key, _, _)| target.contains(key))
                    .map(|(_, ctype, body)| (*ctype, body.clone()))
                    .unwrap_or(("text/plain", b"not found".to_vec()));

                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            });
        }
    });

    format!("http://{address}")
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut request = String::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                request.push_str(&String::from_utf8_lossy(&buffer[..count]));
                if request.contains("\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    request
}

// ---------------------------------------------------------------------------
// Vastausten rungot
// ---------------------------------------------------------------------------

const CAPABILITIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<WFS_Capabilities xmlns="http://www.opengis.net/wfs/2.0" version="2.0.0">
    <FeatureTypeList>
        <FeatureType>
            <Name>mv:tk2</Name>
            <Title>Metsävarakuvion kuvio- ja puustotiedot</Title>
        </FeatureType>
    </FeatureTypeList>
</WFS_Capabilities>"#;

/// Kiinteistön palsta WGS84-koordinaatistossa.
fn property_body() -> Vec<u8> {
    serde_json::json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {"kiinteistotunnus": "74055500020000"},
            "geometry": {"type": "Polygon", "coordinates": [[
                [24.900, 60.200], [24.901, 60.200], [24.901, 60.201],
                [24.900, 60.201], [24.900, 60.200]
            ]]}
        }]
    })
    .to_string()
    .into_bytes()
}

/// Kuvio, joka kattaa koko Suomen TM35FIN-koordinaateissa, joten kiinteistö
/// leikkaa sitä varmasti.
fn stand_body() -> Vec<u8> {
    serde_json::json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {
                "standid": 7,
                "standnumber": 1,
                "cuttingtype": 5,
                "cuttingproposalyear": 2030,
                "area": 1.0
            },
            "geometry": {"type": "Polygon", "coordinates": [[
                [100_000.0, 6_000_000.0], [800_000.0, 6_000_000.0],
                [800_000.0, 7_900_000.0], [100_000.0, 7_900_000.0],
                [100_000.0, 6_000_000.0]
            ]]}
        }]
    })
    .to_string()
    .into_bytes()
}

// ---------------------------------------------------------------------------
// GeoPackage
// ---------------------------------------------------------------------------

/// Luo SQLite-tiedoston ohjelman lukeman skeeman mukaan kuviolle 7.
fn create_gpkg(path: &Path) {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE treestand (
                treestandid INTEGER,
                standid INTEGER,
                type INTEGER
            );
            CREATE TABLE treestratum (
                treestandid INTEGER,
                treespecies INTEGER,
                age REAL,
                basalarea REAL,
                stemcount REAL,
                meandiameter REAL,
                meanheight REAL,
                volume REAL,
                sawlogpercent REAL,
                sawlogvolume REAL,
                pulpwoodvolume REAL,
                volumegrowth REAL,
                stembiomass REAL,
                stumpbiomass REAL,
                branchbiomass REAL,
                leafbiomass REAL
            );
            CREATE TABLE treestandsummary (
                treestandid INTEGER,
                meanage REAL,
                basalarea REAL,
                stemcount REAL,
                meandiameter REAL,
                meanheight REAL,
                volume REAL,
                sawlogvolume REAL,
                pulpwoodvolume REAL,
                volumegrowth REAL,
                maintreespecies INTEGER
            );
            CREATE TABLE stand (
                standid INTEGER,
                fertilityclass INTEGER,
                maingroup INTEGER,
                soiltype INTEGER
            );",
        )
        .unwrap();

    connection
        .execute_batch(
            "INSERT INTO treestand VALUES (1, 7, 2);
            INSERT INTO treestratum VALUES
                (1, 2, 55.0, 25.0, 1200.0, 22.0, 16.0, 150.0, 40.0, 60.0, 90.0,
                 15.0, 200.0, 30.0, 40.0, 10.0);
            INSERT INTO treestandsummary VALUES
                (1, 55.0, 25.0, 1200.0, 22.0, 16.0, 150.0, 60.0, 90.0, 15.0, 2);
            INSERT INTO stand VALUES (7, 1, 2, 24);",
        )
        .unwrap();
}

/// Pakkaa annettu tiedosto ZIP-arkistoksi, jollaisena Metsäkeskus sen tarjoaa.
fn zip_gpkg(source: &Path) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("MV_Testila.gpkg", options).unwrap();
        writer.write_all(&fs::read(source).unwrap()).unwrap();
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

// ---------------------------------------------------------------------------
// Binäärin ajaminen
// ---------------------------------------------------------------------------

struct RunOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_binary(
    args: &[&str],
    home: &Path,
    cwd: &Path,
    envs: &[(&str, String)],
    stdin: &str,
) -> RunOutput {
    let mut child = Command::new(env!("CARGO_BIN_EXE_metsaval"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .envs(envs.iter().map(|(key, value)| (*key, value.as_str())))
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binäärin käynnistys epäonnistui");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    RunOutput {
        status: output.status,
        stdout: strip_ansi(&String::from_utf8_lossy(&output.stdout)),
        stderr: strip_ansi(&String::from_utf8_lossy(&output.stderr)),
    }
}

/// Poistaa ANSI-escape-koodit, jotta tulosteen sisältöä voi verrata
/// suoraan ilman värikoodeja.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Valmistelee valetun kotihakemiston: config, tallennettu kunta ja
/// lataushakemisto.
fn prepare_home(dir: &TempDir) -> PathBuf {
    let home = dir.file("home");
    fs::create_dir_all(home.join(".config/metsavaluaatio")).unwrap();
    fs::write(
        home.join(".config/metsavaluaatio/config.toml"),
        "api_key = \"testikey\"\n",
    )
    .unwrap();

    fs::create_dir_all(home.join(".local/share/metsavaluaatio")).unwrap();
    fs::write(
        home.join(".local/share/metsavaluaatio/municipalities.json"),
        r#"{"740":"Testila"}"#,
    )
    .unwrap();

    let downloads = home.join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(
        home.join(".config/user-dirs.dirs"),
        format!("XDG_DOWNLOAD_DIR=\"{}\"\n", downloads.display()),
    )
    .unwrap();

    home
}

fn backend_env(base: &str) -> Vec<(&'static str, String)> {
    vec![
        ("METSAVAL_MML_BASE_URL", format!("{base}/mml")),
        ("METSAVAL_WFS_URL", format!("{base}/wfs")),
        ("METSAVAL_GPKG_BASE_URL", format!("{base}/zip/MV_")),
    ]
}

/// Etsii hakemistopuusta ensimmäisen tiedoston, jonka nimi päättyy annettuun
/// päätteeseen.
fn find_with_suffix(root: &Path, suffix: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(root).ok()? {
        let path = entry.ok()?.path();
        if path.is_dir() {
            if let Some(found) = find_with_suffix(&path, suffix) {
                return Some(found);
            }
        } else if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(suffix))
        {
            return Some(path);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Testit
// ---------------------------------------------------------------------------

#[test]
fn full_run_downloads_the_gpkg_and_writes_both_files() {
    let dir = TempDir::new("full");
    let home = prepare_home(&dir);

    let gpkg = dir.file("MV_Testila.gpkg");
    create_gpkg(&gpkg);

    let base = serve(vec![
        ("/mml", "application/json", property_body()),
        (
            "request=GetCapabilities",
            "application/xml",
            CAPABILITIES.as_bytes().to_vec(),
        ),
        ("request=GetFeature", "application/json", stand_body()),
        ("/zip", "application/zip", zip_gpkg(&gpkg)),
    ]);

    let gpkg_dir = dir.file("gpkg");
    let output = run_binary(
        &[
            "740-555-0002-0000",
            "--gpkg-kohde",
            gpkg_dir.to_str().unwrap(),
        ],
        &home,
        dir.path(),
        &backend_env(&base),
        "y\n",
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(
        output
            .stdout
            .contains("Kiinteistötunnus: 740-555-0002-0000")
    );
    assert!(output.stdout.contains("Kunta: Testila"));
    assert!(
        output
            .stdout
            .contains("Haetaan GeoPackage kunnalle Testila")
    );
    assert!(output.stdout.contains("GeoPackage valmis"));
    assert!(output.stdout.contains("Google maps taso tallennettu"));
    assert!(output.stdout.contains("Excel-yhteenveto tallennettu"));

    assert!(
        find_with_suffix(dir.path(), ".kml").is_some(),
        "KML-tiedostoa ei kirjoitettu"
    );
    assert!(
        find_with_suffix(dir.path(), ".xlsx").is_some(),
        "Excel-tiedostoa ei kirjoitettu"
    );
}

#[test]
fn run_with_an_existing_gpkg_file_skips_the_download() {
    let dir = TempDir::new("existing");
    let home = prepare_home(&dir);

    let gpkg = dir.file("existing.gpkg");
    create_gpkg(&gpkg);

    // Ei /zip-reittiä: jos ohjelma yrittäisi ladata, se saisi 404-tyylisen
    // vastauksen eikä ajo onnistuisi.
    let base = serve(vec![
        ("/mml", "application/json", property_body()),
        (
            "request=GetCapabilities",
            "application/xml",
            CAPABILITIES.as_bytes().to_vec(),
        ),
        ("request=GetFeature", "application/json", stand_body()),
    ]);

    let output = run_binary(
        &[
            "740-555-0002-0000",
            "--gpkg-tiedosto",
            gpkg.to_str().unwrap(),
        ],
        &home,
        dir.path(),
        &backend_env(&base),
        "y\n",
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(!output.stdout.contains("Haetaan GeoPackage"));
    assert!(find_with_suffix(dir.path(), ".kml").is_some());
    assert!(find_with_suffix(dir.path(), ".xlsx").is_some());
}

#[test]
fn declining_the_confirmation_writes_no_files() {
    let dir = TempDir::new("decline");
    let home = prepare_home(&dir);

    let gpkg = dir.file("MV_Testila.gpkg");
    create_gpkg(&gpkg);

    let base = serve(vec![
        ("/mml", "application/json", property_body()),
        (
            "request=GetCapabilities",
            "application/xml",
            CAPABILITIES.as_bytes().to_vec(),
        ),
        ("request=GetFeature", "application/json", stand_body()),
        ("/zip", "application/zip", zip_gpkg(&gpkg)),
    ]);

    let output = run_binary(
        &["740-555-0002-0000"],
        &home,
        dir.path(),
        &backend_env(&base),
        "n\n",
    );

    assert!(output.status.success(), "stderr:\n{}", output.stderr);
    assert!(find_with_suffix(dir.path(), ".kml").is_none());
    assert!(find_with_suffix(dir.path(), ".xlsx").is_none());
}

#[test]
fn a_property_without_stands_fails_with_a_clear_message() {
    let dir = TempDir::new("nostands");
    let home = prepare_home(&dir);

    let base = serve(vec![
        ("/mml", "application/json", property_body()),
        (
            "request=GetCapabilities",
            "application/xml",
            CAPABILITIES.as_bytes().to_vec(),
        ),
        (
            "request=GetFeature",
            "application/json",
            br#"{"type":"FeatureCollection","features":[]}"#.to_vec(),
        ),
        ("/zip", "application/zip", Vec::new()),
    ]);

    let output = run_binary(
        &["740-555-0002-0000"],
        &home,
        dir.path(),
        &backend_env(&base),
        "y\n",
    );

    assert!(!output.status.success());
    assert!(
        output.stdout.contains("ei löytynyt yhtään metsäkuviota")
            || output.stderr.contains("ei löytynyt yhtään metsäkuviota"),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(find_with_suffix(dir.path(), ".kml").is_none());
}

#[test]
fn an_unknown_municipality_is_asked_and_saved() {
    let dir = TempDir::new("newmunicipality");
    let home = prepare_home(&dir);
    let municipalities = home.join(".local/share/metsavaluaatio/municipalities.json");
    fs::remove_file(&municipalities).unwrap();

    let gpkg = dir.file("MV_Testila.gpkg");
    create_gpkg(&gpkg);

    let base = serve(vec![
        ("/mml", "application/json", property_body()),
        (
            "request=GetCapabilities",
            "application/xml",
            CAPABILITIES.as_bytes().to_vec(),
        ),
        ("request=GetFeature", "application/json", stand_body()),
        ("/zip", "application/zip", zip_gpkg(&gpkg)),
    ]);

    let gpkg_dir = dir.file("gpkg");
    // Ensin vastaus kuntakysymykseen, sitten vahvistus tiedostojen luontiin.
    let output = run_binary(
        &[
            "740-555-0002-0000",
            "--gpkg-kohde",
            gpkg_dir.to_str().unwrap(),
        ],
        &home,
        dir.path(),
        &backend_env(&base),
        "Uusikaupunki\ny\n",
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(output.stdout.contains("Syötä kunta:"));
    assert!(output.stdout.contains("Kunta: Uusikaupunki"));

    let saved = fs::read_to_string(&municipalities).unwrap();
    assert!(saved.contains("Uusikaupunki"), "tallennettu: {saved}");
}

#[test]
fn help_and_missing_arguments_are_handled() {
    let dir = TempDir::new("help");
    let home = dir.file("home");
    fs::create_dir_all(&home).unwrap();

    let help = run_binary(&["--help"], &home, dir.path(), &[], "");
    assert!(help.status.success());
    assert!(help.stdout.contains("METSÄVALUAATIO"));

    let usage = run_binary(&[], &home, dir.path(), &[], "");
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stderr.contains("Käyttö:"));
}

#[test]
fn api_key_and_municipality_commands_work() {
    let dir = TempDir::new("commands");
    let home = dir.file("home");
    fs::create_dir_all(&home).unwrap();

    let set = run_binary(&["--aseta-api", "avain123"], &home, dir.path(), &[], "");
    assert!(set.status.success());
    let config = fs::read_to_string(home.join(".config/metsavaluaatio/config.toml")).unwrap();
    assert!(config.contains("api_key = \"avain123\""));

    let list = run_binary(&["--kunta-lista"], &home, dir.path(), &[], "");
    assert!(list.status.success());

    let remove = run_binary(&["--poista-kunta", "740"], &home, dir.path(), &[], "");
    assert!(remove.status.success());
    assert!(remove.stdout.contains("poistettu"));
}
