use std::path::PathBuf;

use crate::consts::{Cutting, PRICES, Species, TimberType};
use crate::model::{RawRow, SpeciesSummary, StandValue, TimberValue, Totals};
use color_eyre::Result;
use rust_xlsxwriter::{Format, FormatAlign, Workbook};

/// Excel-solu.
enum Cell {
    Text(String),
    Int(i64),
    Num(f64),
    Blank,
}

impl Cell {
    /// Solun merkkijonoesityksen pituus sarakeleveyttä varten.
    fn display_len(&self) -> usize {
        match self {
            Cell::Text(text) => text.chars().count(),
            Cell::Int(value) => value.to_string().chars().count(),
            Cell::Num(value) => format!("{value}").chars().count(),
            Cell::Blank => 0,
        }
    }
}

/// Yksi Excel-välilehti.
struct Sheet {
    name: String,
    headers: Vec<String>,
    rows: Vec<Vec<Cell>>,
}

fn text(value: &str) -> Cell {
    Cell::Text(value.to_string())
}

fn opt_num(value: Option<f64>) -> Cell {
    value.map(Cell::Num).unwrap_or(Cell::Blank)
}

fn opt_int(value: Option<i64>) -> Cell {
    value.map(Cell::Int).unwrap_or(Cell::Blank)
}

fn cutting_text(cutting: Option<Cutting>) -> Cell {
    match cutting {
        Some(cutting) => text(cutting.name()),
        None => text(""),
    }
}

fn headers(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn write_workbook(path: &PathBuf, sheets: &[Sheet]) -> Result<()> {
    let mut workbook = Workbook::new();
    let header_format = Format::new().set_bold().set_align(FormatAlign::Center);

    for sheet in sheets {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&sheet.name)?;

        for (column, header) in sheet.headers.iter().enumerate() {
            worksheet.write_string_with_format(0, column as u16, header, &header_format)?;
        }

        for (row_index, row) in sheet.rows.iter().enumerate() {
            for (column, cell) in row.iter().enumerate() {
                let row = (row_index + 1) as u32;
                let column = column as u16;

                match cell {
                    Cell::Text(value) => {
                        worksheet.write_string(row, column, value)?;
                    }
                    Cell::Int(value) => {
                        worksheet.write_number(row, column, *value as f64)?;
                    }
                    Cell::Num(value) => {
                        worksheet.write_number(row, column, *value)?;
                    }
                    Cell::Blank => {}
                }
            }
        }

        for column in 0..sheet.headers.len() {
            let mut max_length = sheet.headers[column].chars().count();
            for row in &sheet.rows {
                if let Some(cell) = row.get(column) {
                    max_length = max_length.max(cell.display_len());
                }
            }

            let width = (max_length + 2).clamp(10, 30);
            worksheet.set_column_width(column as u16, width as f64)?;
        }

        worksheet.set_freeze_panes(1, 0)?;

        if !sheet.rows.is_empty() {
            let last_row = sheet.rows.len() as u32;
            let last_column = sheet.headers.len().saturating_sub(1) as u16;
            worksheet.autofilter(0, 0, last_row, last_column)?;
        }
    }

    workbook.save(path)?;

    Ok(())
}

/// Kirjoittaa koko Excel-yhteenvedon (6 välilehteä) annettuun polkuun.
pub fn export(
    path: &PathBuf,
    totals: &Totals,
    stand_values: &[StandValue],
    species_summary: &[SpeciesSummary],
    timber_values: &[TimberValue],
    raw_rows: &[RawRow],
    fixed_costs: &f64,
) -> Result<()> {
    let summary = summary_sheet(totals, fixed_costs);
    let stand_sheet = stand_sheet(stand_values);
    let species_sheet = species_sheet(species_summary);
    let timber_sheet = timber_sheet(timber_values);
    let raw_sheet = raw_sheet(raw_rows);
    let price_sheet = price_sheet();

    write_workbook(
        path,
        &[
            summary,
            stand_sheet,
            species_sheet,
            timber_sheet,
            raw_sheet,
            price_sheet,
        ],
    )
}

/// Kirjoittaa Excel-yhteenvedon levylle.
///
/// Synkroninen `export` ajetaan `spawn_blocking`-taustasäikeessä, jotta se
/// ei blokkaa asynkronista runtimea ja KML-tason kirjoitus voi olla samalla
/// kertaa rinnakkain. Kaikki argumentit siirtyvät omistajuutena, koska
/// taustasäikeen on oltava `'static`. Palauttaa tiedoston polun.
///
/// Katso myös `maps::generate_and_write_kml`.
pub async fn export_async(
    path: PathBuf,
    totals: Totals,
    stand_values: Vec<StandValue>,
    species_summary: Vec<SpeciesSummary>,
    timber_values: Vec<TimberValue>,
    raw_rows: Vec<RawRow>,
    fixed_costs: f64,
) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || -> Result<PathBuf> {
        export(
            &path,
            &totals,
            &stand_values,
            &species_summary,
            &timber_values,
            &raw_rows,
            &fixed_costs,
        )?;
        Ok(path)
    })
    // `.await` on `Result<Result<PathBuf, Report>, JoinError>`: yksi `?`
    // poistaa taustasäikeen `JoinError`in ja muuntaa sen virheeksi,
    // lopputulos on funktion paluuarvo.
    .await?
}

fn summary_sheet(totals: &Totals, fixed_costs: &f64) -> Sheet {
    Sheet {
        name: "Yhteenveto".to_string(),
        headers: headers(&["Tunnusluku", "Arvo"]),
        rows: vec![
            vec![text("Kuvioiden määrä"), Cell::Int(totals.stand_count)],
            vec![text("Pinta-ala, ha"), Cell::Num(totals.area_ha)],
            vec![
                text("Puuston kokonaismäärä, m³"),
                Cell::Num(totals.volume_m3),
            ],
            vec![text("Tukkipuuta, m³"), Cell::Num(totals.sawlog_m3)],
            vec![text("Kuitupuuta, m³"), Cell::Num(totals.pulpwood_m3)],
            vec![
                text("Puuston kokonaiskasvu, m³/v"),
                Cell::Num(totals.growth_m3_per_year),
            ],
            vec![text("Tulot, €"), Cell::Num(totals.income_eur)],
            vec![text("Muuttuvat kulut, €"), Cell::Num(totals.costs_eur)],
            vec![text("Kiinteät kulut, €"), Cell::Num(*fixed_costs)],
            vec![text("Netto, €"), Cell::Num(totals.net_eur - fixed_costs)],
        ],
    }
}

fn stand_sheet(stand_values: &[StandValue]) -> Sheet {
    let headers = headers(&[
        "Kuvio ID",
        "Kuvio",
        "Hakkuuehdotus",
        "Ehdotusvuosi",
        "Keski-ikä, v",
        "Runkoluku, kpl/ha",
        "Keskiläpimitta, cm",
        "Keskipituus, m",
        "Pinta-ala, ha",
        "Tilavuus, m³/ha",
        "Tukkia, m³/ha",
        "Kuitupuuta, m³/ha",
        "Tilavuuskasvu, m³/ha/v",
        "Tilavuuskasvu, %",
        "Tulot, €",
        "Menot, €",
        "Netto, €",
    ]);

    let rows = stand_values
        .iter()
        .map(|row| {
            vec![
                Cell::Int(row.standid),
                opt_int(row.standnumber),
                cutting_text(row.cutting),
                opt_int(row.cuttingproposalyear),
                opt_num(row.meanage),
                opt_num(row.stemcount),
                opt_num(row.meandiameter),
                opt_num(row.meanheight),
                opt_num(row.area),
                Cell::Num(row.volume),
                Cell::Num(row.sawlogvolume),
                Cell::Num(row.pulpwoodvolume),
                Cell::Num(row.volumegrowth),
                opt_num(row.growth_pct),
                Cell::Num(row.income_eur),
                Cell::Num(row.costs_eur),
                Cell::Num(row.net_eur),
            ]
        })
        .collect();

    Sheet {
        name: "Kuvioittain".to_string(),
        headers,
        rows,
    }
}

fn species_sheet(species_summary: &[SpeciesSummary]) -> Sheet {
    let headers = headers(&[
        "Kuvio ID",
        "Kuvio",
        "Puulaji",
        "Hakkuuehdotus",
        "Ehdotusvuosi",
        "Ikä, v",
        "Runkoluku, kpl/ha",
        "Keskiläpimitta, cm",
        "Keskipituus, m",
        "Pinta-ala, ha",
        "Tilavuus, m³/ha",
        "Tukkia, m³/ha",
        "Kuitupuuta, m³/ha",
        "Tilavuuskasvu, m³/ha/v",
        "Tilavuuskasvu, %",
        "Tulot, €",
        "Arvokasvu, %",
    ]);

    let rows = species_summary
        .iter()
        .map(|row| {
            vec![
                Cell::Int(row.standid),
                opt_int(row.standnumber),
                text(row.species.name()),
                cutting_text(row.cutting),
                opt_int(row.cuttingproposalyear),
                opt_num(row.age),
                Cell::Num(row.stemcount),
                opt_num(row.meandiameter),
                opt_num(row.meanheight),
                opt_num(row.area),
                Cell::Num(row.volume),
                Cell::Num(row.sawlogvolume),
                Cell::Num(row.pulpwoodvolume),
                Cell::Num(row.volumegrowth),
                opt_num(row.growth_pct),
                Cell::Num(row.value_eur),
                opt_num(row.value_growth_pct),
            ]
        })
        .collect();

    Sheet {
        name: "Puulajeittain".to_string(),
        headers,
        rows,
    }
}

fn timber_sheet(timber_values: &[TimberValue]) -> Sheet {
    let headers = headers(&[
        "Kuvio ID",
        "Kuvio",
        "Puulaji",
        "Hakkuuehdotus",
        "Ehdotusvuosi",
        "Ikä, v",
        "Puutavaralaji",
        "Tilavuus, m³/ha",
    ]);

    let rows = timber_values
        .iter()
        .map(|row| {
            vec![
                Cell::Int(row.standid),
                opt_int(row.standnumber),
                text(row.species.name()),
                cutting_text(row.cutting),
                opt_int(row.cuttingproposalyear),
                opt_num(row.age),
                text(row.timber_type.name()),
                Cell::Num(row.volume),
            ]
        })
        .collect();

    Sheet {
        name: "Puutavaralajeittain".to_string(),
        headers,
        rows,
    }
}

fn raw_sheet(raw_rows: &[RawRow]) -> Sheet {
    let headers = headers(&[
        "Kuvio",
        "Kuvio ID",
        "Puulaji ID",
        "Ikä, v",
        "basalarea",
        "Runkoluku, kpl/ha",
        "Keskiläpimitta, cm",
        "Keskipituus, m",
        "Tilavuus, m³/ha",
        "Tukkiprosentti, %",
        "Tukkia, m³/ha",
        "Kuitupuuta, m³/ha",
        "Tilavuuskasvu, m³/ha/v",
        "Runkobiomassa",
        "Kantobiomassa",
        "Oksabiomassa",
        "Lehtibiomassa",
        "Puulaji",
        "Hakkuuehdotus",
        "Ehdotusvuosi",
        "Kasvupaikka",
        "Maaluokka",
        "Maalaji",
    ]);

    let rows = raw_rows
        .iter()
        .map(|row| {
            vec![
                opt_int(row.standnumber),
                Cell::Int(row.standid),
                Cell::Int(row.treespecies),
                opt_num(row.age),
                opt_num(row.basalarea),
                opt_num(row.stemcount),
                opt_num(row.meandiameter),
                opt_num(row.meanheight),
                opt_num(row.volume),
                opt_num(row.sawlogpercent),
                opt_num(row.sawlogvolume),
                opt_num(row.pulpwoodvolume),
                opt_num(row.volumegrowth),
                opt_num(row.stembiomass),
                opt_num(row.stumpbiomass),
                opt_num(row.branchbiomass),
                opt_num(row.leafbiomass),
                text(row.species.name()),
                cutting_text(row.cutting),
                opt_int(row.cuttingproposalyear),
                text(row.fertility_class.name()),
                text(row.main_group.name()),
                text(row.soil_type.name()),
            ]
        })
        .collect();

    Sheet {
        name: "Raakadata".to_string(),
        headers,
        rows,
    }
}

fn price_sheet() -> Sheet {
    let headers = headers(&["Puulaji", "Puutavaralaji", "Hakkuutapa", "Hinta, €/m³"]);

    let mut rows = Vec::new();

    for species in [Species::Manty, Species::Kuusi, Species::Koivu] {
        for timber in [TimberType::Tukki, TimberType::Kuitu] {
            for cutting in [
                Cutting::Uudistus,
                Cutting::Harvennus,
                Cutting::Ensiharvennus,
            ] {
                rows.push(vec![
                    text(species.name()),
                    text(timber.name()),
                    text(cutting.name()),
                    Cell::Num(PRICES.get(species, timber, cutting)),
                ]);
            }
        }
    }

    Sheet {
        name: "Hinnasto".to_string(),
        headers,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RawRow, SpeciesSummary, TimberValue};
    use std::fs::File;
    use std::io::Read;

    fn totals() -> Totals {
        Totals {
            stand_count: 3,
            area_ha: 1.5,
            volume_m3: 100.0,
            sawlog_m3: 40.0,
            pulpwood_m3: 60.0,
            growth_m3_per_year: 10.0,
            income_eur: 1000.0,
            costs_eur: 200.0,
            net_eur: 800.0,
        }
    }

    fn stand_value() -> StandValue {
        StandValue {
            standid: 11,
            standnumber: Some(7),
            cutting: Some(Cutting::Uudistus),
            cuttingproposalyear: Some(2030),
            volume: 150.0,
            sawlogvolume: 60.0,
            pulpwoodvolume: 90.0,
            volumegrowth: 15.0,
            meanage: Some(55.0),
            stemcount: Some(1200.0),
            meandiameter: Some(22.0),
            meanheight: Some(16.0),
            area: Some(1.5),
            growth_pct: Some(10.0),
            main_species: Some(2),
            fertility_class: crate::consts::FertilityClass::Lehto,
            main_group: crate::consts::MainGroup::MetsaMaa,
            soil_type: crate::consts::SoilType::Savimaa,
            income_eur: 1000.0,
            costs_eur: 200.0,
            net_eur: 800.0,
        }
    }

    fn species_summary() -> SpeciesSummary {
        SpeciesSummary {
            standid: 11,
            standnumber: Some(7),
            species: Species::Kuusi,
            cutting: Some(Cutting::Uudistus),
            cuttingproposalyear: Some(2030),
            age: Some(55.0),
            stemcount: 1200.0,
            meandiameter: Some(22.0),
            meanheight: Some(16.0),
            area: Some(1.5),
            volume: 150.0,
            sawlogvolume: 60.0,
            pulpwoodvolume: 90.0,
            volumegrowth: 15.0,
            growth_pct: Some(10.0),
            value_growth_pct: Some(12.5),
            value_eur: 1000.0,
        }
    }

    fn timber_value() -> TimberValue {
        TimberValue {
            standid: 11,
            standnumber: Some(7),
            species: Species::Kuusi,
            cutting: Some(Cutting::Uudistus),
            cuttingproposalyear: Some(2030),
            volume: 60.0,
            age: Some(55.0),
            timber_type: TimberType::Tukki,
        }
    }

    fn raw_row() -> RawRow {
        RawRow {
            standid: 11,
            treespecies: 2,
            species: Species::Kuusi,
            age: Some(55.0),
            basalarea: Some(25.0),
            stemcount: Some(1200.0),
            meandiameter: Some(22.0),
            meanheight: Some(16.0),
            volume: Some(150.0),
            sawlogpercent: Some(40.0),
            sawlogvolume: Some(60.0),
            pulpwoodvolume: Some(90.0),
            volumegrowth: Some(15.0),
            stembiomass: Some(200.0),
            stumpbiomass: Some(30.0),
            branchbiomass: Some(40.0),
            leafbiomass: Some(10.0),
            standnumber: Some(7),
            cutting: Some(Cutting::Uudistus),
            cuttingproposalyear: Some(2030),
            fertility_class: crate::consts::FertilityClass::Lehto,
            main_group: crate::consts::MainGroup::MetsaMaa,
            soil_type: crate::consts::SoilType::Savimaa,
        }
    }

    /// Kirjoittaa yhden täyden työkirjan ja palauttaa sen polun
    fn export_workbook(label: &str) -> (crate::test_support::TempDir, PathBuf) {
        let dir = crate::test_support::TempDir::new(label);
        assert!(dir.path().exists(), "väliaikaishakemistoa ei luotu");
        let path = dir.file("test.xlsx");
        export(
            &path,
            &totals(),
            &[stand_value()],
            &[species_summary()],
            &[timber_value()],
            &[raw_row()],
            &109.0,
        )
        .unwrap();
        assert!(path.exists(), "työkirjaa ei kirjoitettu");
        (dir, path)
    }

    fn read_entry(path: &PathBuf, name: &str) -> String {
        let file = File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut content = String::new();
        if archive.by_name(name).is_err() {
            let mut names = Vec::new();
            for index in 0..archive.len() {
                names.push(archive.by_index(index).unwrap().name().unwrap().to_string());
            }
            panic!("arkistossa ei ole sivakohdetta {name}: {names:?}");
        }

        let mut entry = archive.by_name(name).unwrap();
        entry.read_to_string(&mut content).unwrap();
        content
    }

    fn entry_names(path: &PathBuf) -> Vec<String> {
        let file = File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect()
    }

    #[test]
    fn export_writes_a_readable_xlsx_archive() {
        let (dir, path) = export_workbook("excel_archive");
        let names = entry_names(&path);
        assert!(names.contains(&"[Content_Types].xml".to_string()));
        drop(dir);
    }

    #[test]
    fn workbook_contains_all_six_sheets_in_order() {
        let (dir, path) = export_workbook("excel_sheets");
        let workbook = read_entry(&path, "xl/workbook.xml");
        let sheet_names: Vec<&str> = workbook
            .split("<sheet name=\"")
            .skip(1)
            .filter_map(|part| part.split('"').next())
            .collect();
        assert_eq!(
            sheet_names,
            vec![
                "Yhteenveto",
                "Kuvioittain",
                "Puulajeittain",
                "Puutavaralajeittain",
                "Raakadata",
                "Hinnasto",
            ]
        );
        drop(dir);
    }

    #[test]
    fn summary_sheet_contains_the_key_figures() {
        let (dir, path) = export_workbook("excel_summary");
        let sheet = read_entry(&path, "xl/worksheets/sheet1.xml");

        // Kuvioiden määrä on kokonaislukuna
        assert!(sheet.contains(">3</v>"), "kuvioiden määrä puuttuu: {sheet}");
        // Pinta-ala 1,5 ha ja tulot 1000 €
        assert!(sheet.contains(">1.5</v>"));
        assert!(sheet.contains(">1000</v>"));
        // Netto = 800 - 109 kiinteät kulut
        assert!(sheet.contains(">691</v>"));

        let strings = read_entry(&path, "xl/sharedStrings.xml");
        for header in ["Tunnusluku", "Kuvioiden määrä", "Pinta-ala, ha", "Netto, €"] {
            assert!(strings.contains(header), "otsikko {header} puuttuu");
        }
        drop(dir);
    }

    #[test]
    fn stand_sheet_lists_the_stand_row() {
        let (dir, path) = export_workbook("excel_stand");
        let sheet = read_entry(&path, "xl/worksheets/sheet2.xml");
        assert!(sheet.contains(">11</v>"), "kuvio ID puuttuu");
        assert!(sheet.contains(">150</v>"), "tilavuus puuttuu");

        let strings = read_entry(&path, "xl/sharedStrings.xml");
        for header in ["Kuvio ID", "Hakkuuehdotus", "Netto, €"] {
            assert!(strings.contains(header), "otsikko {header} puuttuu");
        }
        drop(dir);
    }

    #[test]
    fn every_sheet_has_rows_for_the_given_data() {
        let (dir, path) = export_workbook("excel_rows");
        // 6 välilehteä: otsikkorivi + datarivi
        for sheet_number in 1..=6 {
            let sheet = read_entry(&path, &format!("xl/worksheets/sheet{sheet_number}.xml"));
            assert!(
                sheet.contains("<row r=\"2\""),
                "välilehdeltä {sheet_number} puuttuu datarivi"
            );
        }
        drop(dir);
    }

    #[test]
    fn price_sheet_lists_every_price() {
        let (dir, path) = export_workbook("excel_prices");
        let strings = read_entry(&path, "xl/sharedStrings.xml");
        for name in ["Mänty", "Kuusi", "Koivu", "tukki", "kuitu", "uudistus"] {
            assert!(strings.contains(name), "hintaa {name} puuttuu");
        }
        drop(dir);
    }

    #[test]
    fn export_works_with_empty_data() {
        let dir = crate::test_support::TempDir::new("excel_empty");
        let path = dir.file("empty.xlsx");
        export(&path, &Totals::default(), &[], &[], &[], &[], &0.0).unwrap();
        assert!(path.exists());

        // Otsikkorivit ja välilehdet kirjoitetaan myös tyhjällä aineistolla
        let workbook = read_entry(&path, "xl/workbook.xml");
        assert!(workbook.contains("name=\"Yhteenveto\""));
        assert!(workbook.contains("name=\"Hinnasto\""));

        // Hinnasto ei silti ole tyhjä
        let prices = read_entry(&path, "xl/worksheets/sheet6.xml");
        assert!(prices.contains("<row r=\"2\""));
    }

    #[tokio::test]
    async fn export_async_writes_the_workbook() {
        let dir = crate::test_support::TempDir::new("excel_async");
        let path = dir.file("async.xlsx");

        let written = export_async(
            path.clone(),
            totals(),
            vec![stand_value()],
            vec![species_summary()],
            vec![timber_value()],
            vec![raw_row()],
            109.0,
        )
        .await
        .unwrap();

        assert_eq!(written, path, "palautettava polku on annettu polku");
        let workbook = read_entry(&path, "xl/workbook.xml");
        assert!(workbook.contains("name=\"Yhteenveto\""));
        assert!(workbook.contains("name=\"Kuvioittain\""));
    }

    #[test]
    fn cell_display_lengths_are_character_counts() {
        assert_eq!(Cell::Text("abc".to_string()).display_len(), 3);
        assert_eq!(Cell::Text("äö".to_string()).display_len(), 2);
        assert_eq!(Cell::Int(-1234).display_len(), 5);
        assert_eq!(Cell::Num(1.5).display_len(), 3);
        assert_eq!(Cell::Num(100.0).display_len(), 3);
        assert_eq!(Cell::Blank.display_len(), 0);
    }

    #[test]
    fn optional_cells_fall_back_to_blank() {
        assert!(matches!(opt_num(None), Cell::Blank));
        assert!(matches!(opt_int(None), Cell::Blank));
        assert!(matches!(opt_num(Some(1.5)), Cell::Num(value) if value == 1.5));
        assert!(matches!(opt_int(Some(3)), Cell::Int(value) if value == 3));
        assert!(matches!(cutting_text(None), Cell::Text(value) if value.is_empty()));
        assert!(matches!(
            cutting_text(Some(Cutting::Harvennus)),
            Cell::Text(value) if value == "harvennus"
        ));
    }
}
