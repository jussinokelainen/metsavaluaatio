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

            let width = ((max_length + 2).max(10)).min(30);
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
) -> Result<()> {
    let summary = summary_sheet(totals);
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

fn summary_sheet(totals: &Totals) -> Sheet {
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
            vec![
                text("Laskennallinen puuston arvo, €"),
                Cell::Num(totals.value_eur),
            ],
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
        "Arvo, €",
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
                Cell::Num(row.value_eur),
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
        "Arvo, €",
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
        "Kuvio",
        "Hakkuuehdotus",
        "Ehdotusvuosi",
    ]);

    let rows = raw_rows
        .iter()
        .map(|row| {
            vec![
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
                opt_int(row.standnumber),
                cutting_text(row.cutting),
                opt_int(row.cuttingproposalyear),
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
