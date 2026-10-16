use crate::consts::{GREEN, MUNICIPALITY_ZIP_BASE, RESET};
use crate::http;
use crate::model::{StandSummary, TreeStratum};
use color_eyre::Result;
use rusqlite::{Connection, params_from_iter};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Hakee kunnan metsävarakuviot Metsäkeskuksesta ja purkaa GeoPackagen
/// väliaikaistiedostoon.
pub fn download_gpkg(municipality_name: &str) -> Result<PathBuf> {
    println!("Haetaan GeoPackage kunnalle {municipality_name}...");
    let url = format!("{MUNICIPALITY_ZIP_BASE}{municipality_name}.zip");
    let bytes = http::get_bytes(&url, &[], 300)?;

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

    let temp_dir = std::env::temp_dir().join(format!("metsavaluaatio_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    let gpkg_path = temp_dir.join(file_name);
    let mut output = fs::File::create(&gpkg_path)?;
    std::io::copy(&mut entry, &mut output)?;

    println!("{GREEN}GeoPackage valmis{RESET}: {}", gpkg_path.display());

    Ok(gpkg_path)
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
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
            tss.volumegrowth
        FROM treestand ts
        JOIN treestandsummary tss ON tss.treestandid = ts.treestandid
        WHERE ts.standid IN ({placeholders})
          AND ts.type = 2"
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
        })
    })?;

    let mut summaries = Vec::new();
    for row in rows {
        summaries.push(row?);
    }

    Ok((strata, summaries))
}
