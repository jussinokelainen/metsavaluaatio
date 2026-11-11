use crate::consts::municipality_zip_base;
use crate::http;
use crate::model::{StandSummary, TreeStratum};
use color_eyre::Result;
use rusqlite::{Connection, params_from_iter};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Hakee kunnan metsävarakuviot Metsäkeskuksesta ja purkaa GeoPackagen
/// väliaikaistiedostoon.
pub async fn download_gpkg(municipality_name: &str, dest_dir: PathBuf) -> Result<PathBuf> {
    let url = format!("{}{municipality_name}.zip", municipality_zip_base());
    download_gpkg_from(&url, dest_dir).await
}

/// Hakee ZIP-arkiston annetusta osoitteesta ja purkaa siitä GeoPackagen.
async fn download_gpkg_from(url: &str, dest_dir: PathBuf) -> Result<PathBuf> {
    let bytes = http::get_bytes(url, &[], 300).await?;
    // Purku kirjoittaa levylle, joten se ajetaan taustasäikeessä
    tokio::task::spawn_blocking(move || extract_gpkg(&bytes, &dest_dir)).await?
}

/// Purkaa ZIP-arkistosta ensimmäisen GeoPackagen (`*.gpkg`) annettuun
/// hakemistoon ja palauttaa purketun tiedoston polun.
fn extract_gpkg(bytes: &[u8], dest_dir: &Path) -> Result<PathBuf> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;

    let mut gpkg_index = None;
    for index in 0..archive.len() {
        let name = {
            let file = archive.by_index(index)?;
            file.name()?.to_string()
        };

        if name.to_lowercase().ends_with(".gpkg") {
            gpkg_index = Some(index);
            break;
        }
    }

    let Some(index) = gpkg_index else {
        color_eyre::eyre::bail!("ZIP-tiedostosta ei löytynyt GeoPackagea.");
    };

    let mut entry = archive.by_index(index)?;
    let name = entry.name()?.to_string();
    let file_name = Path::new(&name)
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("metsavarat.gpkg"));

    fs::create_dir_all(dest_dir)?;

    let gpkg_path = dest_dir.join(file_name);
    let mut output = fs::File::create(&gpkg_path)?;
    std::io::copy(&mut entry, &mut output)?;

    Ok(gpkg_path)
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

/// Asynkroninen `read_trees`. SQLite-luku on synkronista, joten se ajetaan
/// taustasäikeessä eikä se blokkaa asynkronista runtimea.
pub async fn read_trees_async(
    gpkg_path: PathBuf,
    stand_ids: Vec<i64>,
) -> Result<(Vec<TreeStratum>, Vec<StandSummary>)> {
    tokio::task::spawn_blocking(move || read_trees(&gpkg_path, &stand_ids)).await?
}

/// Hakee kuvioiden nykyisen puuston ja koko kuvion yhteenvedot GeoPackagesta.
pub fn read_trees(
    gpkg_path: &Path,
    stand_ids: &[i64],
) -> Result<(Vec<TreeStratum>, Vec<StandSummary>)> {
    if stand_ids.is_empty() {
        color_eyre::eyre::bail!("Ei kuvioita, joille hakea puustotietoja.");
    }

    let connection = Connection::open(gpkg_path)?;
    let placeholders = placeholders(stand_ids.len());

    let strata_sql = format!(
        "SELECT
            ts.standid,
            tr.treespecies,
            tr.age,
            tr.basalarea,
            tr.stemcount,
            tr.meandiameter,
            tr.meanheight,
            tr.volume,
            tr.sawlogpercent,
            tr.sawlogvolume,
            tr.pulpwoodvolume,
            tr.volumegrowth,
            tr.stembiomass,
            tr.stumpbiomass,
            tr.branchbiomass,
            tr.leafbiomass
        FROM treestand ts
        JOIN treestratum tr ON tr.treestandid = ts.treestandid
        WHERE ts.standid IN ({placeholders})
          AND ts.type = 2"
    );

    let mut statement = connection.prepare(&strata_sql)?;
    let rows = statement.query_map(params_from_iter(stand_ids.iter().copied()), |row| {
        Ok(TreeStratum {
            standid: row.get(0)?,
            treespecies: row.get(1)?,
            age: row.get(2)?,
            basalarea: row.get(3)?,
            stemcount: row.get(4)?,
            meandiameter: row.get(5)?,
            meanheight: row.get(6)?,
            volume: row.get(7)?,
            sawlogpercent: row.get(8)?,
            sawlogvolume: row.get(9)?,
            pulpwoodvolume: row.get(10)?,
            volumegrowth: row.get(11)?,
            stembiomass: row.get(12)?,
            stumpbiomass: row.get(13)?,
            branchbiomass: row.get(14)?,
            leafbiomass: row.get(15)?,
        })
    })?;

    let mut strata = Vec::new();
    for row in rows {
        strata.push(row?);
    }

    let summary_sql = format!(
        "SELECT
            ts.standid,
            tss.meanage,
            tss.basalarea,
            tss.stemcount,
            tss.meandiameter,
            tss.meanheight,
            tss.volume,
            tss.sawlogvolume,
            tss.pulpwoodvolume,
            tss.volumegrowth,
            species.maintreespecies,
            s.fertilityclass,
            s.maingroup,
            s.soiltype
        FROM treestand ts
        JOIN treestandsummary tss
            ON tss.treestandid = ts.treestandid
        LEFT JOIN (
            SELECT
                ts2.standid,
                MAX(tss2.maintreespecies) AS maintreespecies
            FROM treestand ts2
            JOIN treestandsummary tss2
                ON tss2.treestandid = ts2.treestandid
            WHERE tss2.maintreespecies IS NOT NULL
            GROUP BY ts2.standid
        ) species ON species.standid = ts.standid
        LEFT JOIN stand s
            ON s.standid = ts.standid
        WHERE ts.standid IN ({placeholders})
          AND ts.type = 2;"
    );

    let mut statement = connection.prepare(&summary_sql)?;
    let rows = statement.query_map(params_from_iter(stand_ids.iter().copied()), |row| {
        Ok(StandSummary {
            standid: row.get(0)?,
            meanage: row.get(1)?,
            basalarea: row.get(2)?,
            stemcount: row.get(3)?,
            meandiameter: row.get(4)?,
            meanheight: row.get(5)?,
            volume: row.get(6)?,
            sawlogvolume: row.get(7)?,
            pulpwoodvolume: row.get(8)?,
            volumegrowth: row.get(9)?,
            main_species: row.get(10)?,
            fertility_class: row.get(11)?,
            main_group: row.get(12)?,
            soil_type: row.get(13)?,
        })
    })?;

    let mut summaries = Vec::new();
    for row in rows {
        summaries.push(row?);
    }

    Ok((strata, summaries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TempDir, serve_bytes};
    use std::io::Write;

    #[test]
    fn placeholders_are_comma_separated_question_marks() {
        assert_eq!(placeholders(0), "");
        assert_eq!(placeholders(1), "?");
        assert_eq!(placeholders(3), "?,?,?");
    }

    /// Rakentaa muistissa olevan ZIP-arkiston annetuista tiedostoista.
    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn extract_gpkg_extracts_the_first_gpkg_entry() {
        let dir = TempDir::new("extract");
        let dest = dir.file("kohde");
        assert!(!dest.exists(), "kohdemistoa ei ole vielä olemassa");

        let bytes = build_zip(&[
            ("lue-minut.txt", b"ei geopackagea"),
            ("MV_Test.gpkg", b"gpkg-sisalto"),
        ]);
        let path = extract_gpkg(&bytes, &dest).unwrap();

        // Polku muodostetaan kohdehakemiston perään ja se luodaan
        assert_eq!(path, dest.join("MV_Test.gpkg"));
        assert_eq!(std::fs::read(&path).unwrap(), b"gpkg-sisalto");
    }

    #[test]
    fn extract_gpkg_matches_the_extension_case_insensitively() {
        let dir = TempDir::new("extract_case");
        let bytes = build_zip(&[("Data.GPKG", b"iso-pilkku")]);
        let path = extract_gpkg(&bytes, dir.path()).unwrap();
        assert_eq!(path, dir.path().join("Data.GPKG"));
        assert_eq!(std::fs::read(&path).unwrap(), b"iso-pilkku");
    }

    #[test]
    fn extract_gpkg_uses_only_the_file_name_of_nested_entries() {
        let dir = TempDir::new("extract_nested");
        let bytes = build_zip(&[("MV_Kuntunimi/kartta.gpkg", b"nested")]);
        let path = extract_gpkg(&bytes, dir.path()).unwrap();
        assert_eq!(path, dir.path().join("kartta.gpkg"));
    }

    #[test]
    fn a_zip_without_a_gpkg_is_an_error() {
        let dir = TempDir::new("extract_none");
        let bytes = build_zip(&[("kuvio.json", b"{}")]);
        assert!(extract_gpkg(&bytes, dir.path()).is_err());
    }

    #[test]
    fn a_corrupt_zip_is_an_error() {
        let dir = TempDir::new("extract_corrupt");
        assert!(extract_gpkg("tämä ei ole zipiä".as_bytes(), dir.path()).is_err());
    }

    #[tokio::test]
    async fn download_gpkg_from_fetches_and_extracts_the_archive() {
        let dir = TempDir::new("download_gpkg");
        let bytes = build_zip(&[("MV_Testi.gpkg", b"verkkosisalto")]);
        let (url, log) = serve_bytes("200 OK", "application/zip", &bytes);

        let dest = dir.file("purettu");
        let path = download_gpkg_from(&url, dest.clone()).await.unwrap();

        assert_eq!(path, dest.join("MV_Testi.gpkg"));
        assert_eq!(std::fs::read(&path).unwrap(), b"verkkosisalto");

        let request = log.lock().unwrap().clone();
        assert!(
            request.lines().next().unwrap().starts_with("GET /"),
            "pyyntölinja: {request}"
        );
    }

    #[tokio::test]
    async fn download_gpkg_from_reports_http_errors() {
        let dir = TempDir::new("download_gpkg_err");
        let (url, _log) = serve_bytes("404 Not Found", "text/plain", "ei ole täällä".as_bytes());
        assert!(
            download_gpkg_from(&url, dir.path().to_path_buf())
                .await
                .is_err()
        );
    }

    /// Luo GeoPackagen kaltaisen SQLite-tiedoston ohjelman lukeman skeeman mukaan.
    fn create_test_database(path: &Path) {
        let connection = Connection::open(path).unwrap();
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

        // Kaksi kuvioita (type 2) ja yksi ei-kuvio (type 1) joka ohitetaan
        connection
            .execute(
                "INSERT INTO treestand (treestandid, standid, type) VALUES
                    (1, 100, 2), (2, 101, 1), (3, 102, 2)",
                [],
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO treestratum (
                    treestandid, treespecies, age, basalarea, stemcount, meandiameter,
                    meanheight, volume, sawlogpercent, sawlogvolume, pulpwoodvolume,
                    volumegrowth, stembiomass, stumpbiomass, branchbiomass, leafbiomass
                ) VALUES
                    (1, 2, 55.5, 25.0, 1200.0, 22.0, 16.0, 150.0, 40.0, 60.0, 90.0,
                     15.0, 200.0, 30.0, 40.0, 10.0),
                    (2, 5, 10.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                     1.0, 1.0, 1.0, 1.0, 1.0),
                    (3, 1, 70.0, NULL, 900.0, 26.0, 18.0, 120.0, 30.0, 36.0, 84.0,
                     12.0, NULL, NULL, NULL, NULL)",
                [],
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO treestandsummary (
                    treestandid, meanage, basalarea, stemcount, meandiameter, meanheight,
                    volume, sawlogvolume, pulpwoodvolume, volumegrowth, maintreespecies
                ) VALUES
                    (1, 55.5, 25.0, 1200.0, 22.0, 16.0, 150.0, 60.0, 90.0, 15.0, 2),
                    (2, 10.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 5),
                    (3, 70.0, NULL, 900.0, 26.0, 18.0, 120.0, 36.0, 84.0, 12.0, NULL)",
                [],
            )
            .unwrap();

        // Vain kuvio 100:llä on riviä stand-taulussa, muut saavat NULL-arvot
        connection
            .execute(
                "INSERT INTO stand (standid, fertilityclass, maingroup, soiltype) VALUES
                    (100, 1, 2, 24)",
                [],
            )
            .unwrap();
    }

    #[test]
    fn read_trees_returns_strata_and_summaries_for_requested_stands() {
        let dir = TempDir::new("metsakeskus");
        let path = dir.file("MV_Test.gpkg");
        create_test_database(&path);

        let (strata, summaries) = read_trees(&path, &[100, 102]).unwrap();

        // type 1 -rivi ohitetaan
        assert_eq!(strata.len(), 2);
        let mut stand_ids: Vec<i64> = strata.iter().map(|row| row.standid).collect();
        stand_ids.sort_unstable();
        assert_eq!(stand_ids, vec![100, 102]);

        let first = strata.iter().find(|row| row.standid == 100).unwrap();
        assert_eq!(first.treespecies, 2);
        assert_eq!(first.age, Some(55.5));
        assert_eq!(first.basalarea, Some(25.0));
        assert_eq!(first.stemcount, Some(1200.0));
        assert_eq!(first.meandiameter, Some(22.0));
        assert_eq!(first.meanheight, Some(16.0));
        assert_eq!(first.volume, Some(150.0));
        assert_eq!(first.sawlogpercent, Some(40.0));
        assert_eq!(first.sawlogvolume, Some(60.0));
        assert_eq!(first.pulpwoodvolume, Some(90.0));
        assert_eq!(first.volumegrowth, Some(15.0));
        assert_eq!(first.stembiomass, Some(200.0));
        assert_eq!(first.leafbiomass, Some(10.0));

        // NULL-arvot tulevat None:ksi
        let second = strata.iter().find(|row| row.standid == 102).unwrap();
        assert_eq!(second.treespecies, 1);
        assert_eq!(second.basalarea, None);
        assert_eq!(second.stembiomass, None);

        assert_eq!(summaries.len(), 2);
        let summary = summaries.iter().find(|row| row.standid == 100).unwrap();
        assert_eq!(summary.meanage, Some(55.5));
        assert_eq!(summary.volume, Some(150.0));
        assert_eq!(summary.main_species, Some(2));
        assert_eq!(summary.fertility_class, Some(1));
        assert_eq!(summary.main_group, Some(2));
        assert_eq!(summary.soil_type, Some(24));

        let missing_stand = summaries.iter().find(|row| row.standid == 102).unwrap();
        assert_eq!(missing_stand.fertility_class, None);
        assert_eq!(missing_stand.main_group, None);
        assert_eq!(missing_stand.soil_type, None);
        assert_eq!(missing_stand.main_species, None);
    }

    #[test]
    fn read_trees_accepts_a_single_stand() {
        let dir = TempDir::new("metsakeskus_single");
        let path = dir.file("MV_Test.gpkg");
        create_test_database(&path);

        let (strata, summaries) = read_trees(&path, &[102]).unwrap();
        assert_eq!(strata.len(), 1);
        assert_eq!(strata[0].standid, 102);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].standid, 102);
    }

    #[tokio::test]
    async fn read_trees_async_returns_the_same_data() {
        let dir = TempDir::new("metsakeskus_async");
        let path = dir.file("MV_Test.gpkg");
        create_test_database(&path);

        let (strata, summaries) = read_trees_async(path, vec![102]).await.unwrap();
        assert_eq!(strata.len(), 1);
        assert_eq!(strata[0].standid, 102);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].standid, 102);
    }

    #[test]
    fn read_trees_of_unknown_stand_returns_nothing() {
        let dir = TempDir::new("metsakeskus_unknown");
        let path = dir.file("MV_Test.gpkg");
        create_test_database(&path);

        let (strata, summaries) = read_trees(&path, &[999]).unwrap();
        assert!(strata.is_empty());
        assert!(summaries.is_empty());
    }

    #[test]
    fn read_trees_without_stand_ids_is_an_error() {
        let dir = TempDir::new("metsakeskus_empty");
        let path = dir.file("MV_Test.gpkg");
        create_test_database(&path);

        assert!(read_trees(&path, &[]).is_err());
    }

    #[test]
    fn read_trees_with_a_broken_database_is_an_error() {
        let dir = TempDir::new("metsakeskus_broken");
        let path = dir.file("broken.gpkg");
        std::fs::write(&path, "tätä ei ole sqlitea").unwrap();

        assert!(read_trees(&path, &[100]).is_err());
    }
}
