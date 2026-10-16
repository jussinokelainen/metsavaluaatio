use crate::consts::{CYAN, GREEN, RESET};
use crate::consts::{Cutting, PRICES, Species, TimberType};
use crate::model::{
    RawRow, SpeciesSummary, Stand, StandSummary, StandValue, TimberValue, Totals, TreeStratum,
};
use color_eyre::Result;
use std::cmp::Ordering;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Apufunktiot
// ---------------------------------------------------------------------------

fn sum(values: impl Iterator<Item = f64>) -> f64 {
    values.sum()
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;

    for value in values {
        total += value;
        count += 1;
    }

    if count == 0 {
        None
    } else {
        Some(total / count as f64)
    }
}

fn cmp_opt_i64(a: Option<i64>, b: Option<i64>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a.cmp(&b),
    }
}

fn cmp_opt_i64_last(a: Option<i64>, b: Option<i64>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => a.cmp(&b),
    }
}

fn cmp_opt_f64(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a.total_cmp(&b),
    }
}

fn cmp_opt_f64_last(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => a.total_cmp(&b),
    }
}

fn cmp_opt_cutting(a: Option<Cutting>, b: Option<Cutting>) -> Ordering {
    a.map(Cutting::name).cmp(&b.map(Cutting::name))
}

// ---------------------------------------------------------------------------
// Puustotietojen yhdistäminen (species_summary)
// ---------------------------------------------------------------------------

struct StratumRow<'a> {
    stratum: &'a TreeStratum,
    stand: &'a Stand,
    species: Species,
}

fn species_key_cmp(a: &StratumRow, b: &StratumRow) -> Ordering {
    a.stratum
        .standid
        .cmp(&b.stratum.standid)
        .then_with(|| cmp_opt_i64(a.stand.standnumber, b.stand.standnumber))
        .then_with(|| a.species.name().cmp(b.species.name()))
        .then_with(|| cmp_opt_cutting(a.stand.cutting(), b.stand.cutting()))
        .then_with(|| cmp_opt_i64(a.stand.cuttingproposalyear, b.stand.cuttingproposalyear))
        .then_with(|| cmp_opt_f64(a.stratum.age, b.stratum.age))
}

/// Yhdistää puustorivit kuvioittain ja puulajeittain (species_summary).
pub fn build_species_summary(
    strata: &[TreeStratum],
    stands: &[Stand],
) -> Result<Vec<SpeciesSummary>> {
    let mut rows: Vec<StratumRow> = Vec::new();
    let mut unknown_species: Vec<i64> = Vec::new();

    for stratum in strata {
        let Some(stand) = stands
            .iter()
            .find(|stand| stand.standid == Some(stratum.standid))
        else {
            continue;
        };

        match Species::from_code(stratum.treespecies) {
            Some(species) => rows.push(StratumRow {
                stratum,
                stand,
                species,
            }),
            None => unknown_species.push(stratum.treespecies),
        }
    }

    if !unknown_species.is_empty() {
        unknown_species.sort_unstable();
        unknown_species.dedup();
        color_eyre::eyre::bail!("Tuntematon puulaji: {unknown_species:?}");
    }

    rows.sort_by(species_key_cmp);

    let mut summaries = Vec::new();

    for chunk in rows.chunk_by(|a, b| species_key_cmp(a, b) == Ordering::Equal) {
        let first = &chunk[0];

        let volume: f64 = sum(chunk.iter().filter_map(|row| row.stratum.volume));
        let sawlogvolume: f64 = sum(chunk.iter().filter_map(|row| row.stratum.sawlogvolume));
        let pulpwoodvolume: f64 = sum(chunk.iter().filter_map(|row| row.stratum.pulpwoodvolume));
        let volumegrowth: f64 = sum(chunk.iter().filter_map(|row| row.stratum.volumegrowth));
        let stemcount: f64 = sum(chunk.iter().filter_map(|row| row.stratum.stemcount));
        let meandiameter = mean(chunk.iter().filter_map(|row| row.stratum.meandiameter));
        let meanheight = mean(chunk.iter().filter_map(|row| row.stratum.meanheight));

        let growth_pct = if volume != 0.0 {
            Some(volumegrowth / volume * 100.0)
        } else {
            None
        };

        let mut summary = SpeciesSummary {
            standid: first.stratum.standid,
            standnumber: first.stand.standnumber,
            species: first.species,
            cutting: first.stand.cutting(),
            cuttingproposalyear: first.stand.cuttingproposalyear,
            age: first.stratum.age,
            stemcount,
            meandiameter,
            meanheight,
            area: first.stand.area,
            volume,
            sawlogvolume,
            pulpwoodvolume,
            volumegrowth,
            growth_pct,
            value_eur: 0.0,
        };

        summary.value_eur = calculate_value(&summary);
        summaries.push(summary);
    }

    // Järjestetään alkuperäisen tulostuksen mukaiseen järjestykseen.
    summaries.sort_by(|a, b| {
        cmp_opt_i64_last(a.standnumber, b.standnumber)
            .then_with(|| a.species.name().cmp(b.species.name()))
            .then_with(|| cmp_opt_f64_last(a.age, b.age))
    });

    Ok(summaries)
}

// ---------------------------------------------------------------------------
// Arvon laskenta
// ---------------------------------------------------------------------------

fn ge(value: Option<f64>, threshold: f64) -> bool {
    value.map(|value| value >= threshold).unwrap_or(false)
}

fn uudistus(species: Species, sawlog: f64, pulpwood: f64, volume: f64, area: f64) -> f64 {
    // -15 % ja 10 % yks. kokonaisvolyymista tukista kuiduksi
    let adjusted_sawlog = area * sawlog * 0.85 - area * volume * 0.1;
    let adjusted_pulpwood = area * pulpwood * 0.85 + area * volume * 0.1;

    adjusted_sawlog * PRICES.get(species, TimberType::Tukki, Cutting::Uudistus)
        + adjusted_pulpwood * PRICES.get(species, TimberType::Kuitu, Cutting::Uudistus)
}

fn harvennus(species: Species, sawlog: f64, _pulpwood: f64, volume: f64, area: f64) -> f64 {
    area * volume * 0.25 * PRICES.get(species, TimberType::Kuitu, Cutting::Harvennus)
        + area * sawlog * 0.15 * PRICES.get(species, TimberType::Tukki, Cutting::Harvennus)
}

/// Laskee puulajikohtaisen arvon (€) kuvion tietojen perusteella.
pub fn calculate_value(row: &SpeciesSummary) -> f64 {
    let species = row.species;
    let volume = row.volume;
    let sawlog = row.sawlogvolume;
    let pulpwood = row.pulpwoodvolume;
    let area = row.area.unwrap_or(0.0);

    match row.cutting {
        Some(Cutting::Ensiharvennus) => return 0.0,
        Some(Cutting::Harvennus) => return harvennus(species, sawlog, pulpwood, volume, area),
        Some(Cutting::Uudistus) => return uudistus(species, sawlog, pulpwood, volume, area),
        None => {}
    }

    match species {
        Species::Kuusi => {
            if ge(row.age, 70.0) || ge(row.meandiameter, 27.0) {
                return uudistus(species, sawlog * 0.6, pulpwood * 0.6, volume * 0.6, area);
            }
            if ge(row.age, 45.0) || ge(row.meandiameter, 18.0) || ge(row.meanheight, 17.0) {
                return harvennus(species, sawlog, pulpwood, volume, area);
            }
        }
        Species::Manty => {
            if ge(row.age, 75.0) || ge(row.meandiameter, 26.0) {
                return uudistus(species, sawlog * 0.6, pulpwood * 0.6, volume * 0.6, area);
            }
            if ge(row.age, 45.0) || ge(row.meandiameter, 17.0) || ge(row.meanheight, 16.0) {
                return harvennus(species, sawlog, pulpwood, volume, area);
            }
        }
        Species::Koivu => {
            if ge(row.age, 60.0) || ge(row.meandiameter, 27.0) {
                return uudistus(species, sawlog * 0.6, pulpwood * 0.6, volume * 0.6, area);
            }
            if ge(row.age, 35.0) || ge(row.meandiameter, 16.0) || ge(row.meanheight, 18.0) {
                return harvennus(species, sawlog, pulpwood, volume, area);
            }
        }
    }

    0.0
}

// ---------------------------------------------------------------------------
// Puutavaralajit ja kuviokohtainen yhteenveto
// ---------------------------------------------------------------------------

/// Rakentaa puulaji + puutavaralaji -rivit (timber_values).
pub fn build_timber_values(species_summary: &[SpeciesSummary]) -> Vec<TimberValue> {
    let mut values = Vec::new();

    for row in species_summary {
        values.push(TimberValue {
            standid: row.standid,
            standnumber: row.standnumber,
            species: row.species,
            cutting: row.cutting,
            cuttingproposalyear: row.cuttingproposalyear,
            volume: row.sawlogvolume,
            age: row.age,
            timber_type: TimberType::Tukki,
        });
    }

    for row in species_summary {
        values.push(TimberValue {
            standid: row.standid,
            standnumber: row.standnumber,
            species: row.species,
            cutting: row.cutting,
            cuttingproposalyear: row.cuttingproposalyear,
            volume: row.pulpwoodvolume,
            age: row.age,
            timber_type: TimberType::Kuitu,
        });
    }

    values
}

/// Rakentaa kuviokohtaisen yhteenvedon (stand_values).
pub fn build_stand_values(
    species_summary: &[SpeciesSummary],
    stand_summaries: &[StandSummary],
    stands: &[Stand],
) -> Vec<StandValue> {
    let mut sorted: Vec<&SpeciesSummary> = species_summary.iter().collect();
    sorted.sort_by(|a, b| {
        a.standid
            .cmp(&b.standid)
            .then_with(|| cmp_opt_i64(a.standnumber, b.standnumber))
            .then_with(|| cmp_opt_cutting(a.cutting, b.cutting))
            .then_with(|| cmp_opt_i64(a.cuttingproposalyear, b.cuttingproposalyear))
    });

    let mut values = Vec::new();

    for chunk in sorted.chunk_by(|a, b| {
        a.standid == b.standid
            && a.standnumber == b.standnumber
            && a.cutting == b.cutting
            && a.cuttingproposalyear == b.cuttingproposalyear
    }) {
        let first = chunk[0];

        let volume: f64 = sum(chunk.iter().map(|row| row.volume));
        let sawlogvolume: f64 = sum(chunk.iter().map(|row| row.sawlogvolume));
        let pulpwoodvolume: f64 = sum(chunk.iter().map(|row| row.pulpwoodvolume));
        let volumegrowth: f64 = sum(chunk.iter().map(|row| row.volumegrowth));
        let value_eur: f64 = sum(chunk.iter().map(|row| row.value_eur));

        let summary = stand_summaries
            .iter()
            .find(|summary| summary.standid == first.standid);

        let area = stands
            .iter()
            .find(|stand| stand.standid == Some(first.standid))
            .and_then(|stand| stand.area);

        let growth_pct = if volume != 0.0 {
            Some(volumegrowth / volume * 100.0)
        } else {
            None
        };

        values.push(StandValue {
            standid: first.standid,
            standnumber: first.standnumber,
            cutting: first.cutting,
            cuttingproposalyear: first.cuttingproposalyear,
            volume,
            sawlogvolume,
            pulpwoodvolume,
            volumegrowth,
            value_eur,
            meanage: summary.and_then(|summary| summary.meanage),
            stemcount: summary.and_then(|summary| summary.stemcount),
            meandiameter: summary.and_then(|summary| summary.meandiameter),
            meanheight: summary.and_then(|summary| summary.meanheight),
            area,
            growth_pct,
        });
    }

    values.sort_by(|a, b| cmp_opt_i64_last(a.standnumber, b.standnumber));

    values
}

/// Rakentaa raakarivit (df): puuston perustiedot + kuvion tiedot.
pub fn build_raw_rows(strata: &[TreeStratum], stands: &[Stand]) -> Result<Vec<RawRow>> {
    let mut rows = Vec::new();

    for stratum in strata {
        let Some(stand) = stands
            .iter()
            .find(|stand| stand.standid == Some(stratum.standid))
        else {
            continue;
        };

        let Some(species) = Species::from_code(stratum.treespecies) else {
            color_eyre::eyre::bail!("Tuntematon puulaji: {}", stratum.treespecies);
        };

        rows.push(RawRow {
            standid: stratum.standid,
            treespecies: stratum.treespecies,
            species,
            age: stratum.age,
            basalarea: stratum.basalarea,
            stemcount: stratum.stemcount,
            meandiameter: stratum.meandiameter,
            meanheight: stratum.meanheight,
            volume: stratum.volume,
            sawlogpercent: stratum.sawlogpercent,
            sawlogvolume: stratum.sawlogvolume,
            pulpwoodvolume: stratum.pulpwoodvolume,
            volumegrowth: stratum.volumegrowth,
            stembiomass: stratum.stembiomass,
            stumpbiomass: stratum.stumpbiomass,
            branchbiomass: stratum.branchbiomass,
            leafbiomass: stratum.leafbiomass,
            standnumber: stand.standnumber,
            cutting: stand.cutting(),
            cuttingproposalyear: stand.cuttingproposalyear,
        });
    }

    Ok(rows)
}

/// Laskee koko aineiston tunnusluvut (Excelin Yhteenveto-välilehti).
pub fn compute_totals(stands: &[Stand], species_summary: &[SpeciesSummary]) -> Totals {
    let stand_ids: BTreeSet<i64> = stands.iter().filter_map(|stand| stand.standid).collect();

    let area_ha = sum(stands.iter().filter_map(|stand| stand.area));

    let volume_m3 = sum(species_summary
        .iter()
        .map(|row| row.volume * row.area.unwrap_or(0.0)));
    let sawlog_m3 = sum(species_summary
        .iter()
        .map(|row| row.sawlogvolume * row.area.unwrap_or(0.0)));
    let pulpwood_m3 = sum(species_summary
        .iter()
        .map(|row| row.pulpwoodvolume * row.area.unwrap_or(0.0)));
    let growth_m3_per_year = sum(species_summary
        .iter()
        .map(|row| row.volumegrowth * row.area.unwrap_or(0.0)));
    let value_eur = sum(species_summary.iter().map(|row| row.value_eur));

    Totals {
        stand_count: stand_ids.len() as i64,
        area_ha,
        volume_m3,
        sawlog_m3,
        pulpwood_m3,
        growth_m3_per_year,
        value_eur,
    }
}

// ---------------------------------------------------------------------------
// Tulostus
// ---------------------------------------------------------------------------

fn grouped(value: f64) -> String {
    let rounded = format!("{value:.0}");
    let (sign, digits) = if let Some(rest) = rounded.strip_prefix('-') {
        ("-", rest.to_string())
    } else {
        ("", rounded)
    };

    let mut result = String::new();
    let chars: Vec<char> = digits.chars().collect();
    for (index, ch) in chars.iter().enumerate() {
        if index > 0 && (chars.len() - index) % 3 == 0 {
            result.push(' ');
        }
        result.push(*ch);
    }

    format!("{sign}{result}")
}

/// Tulostaa koko aineiston yhteenvedon.
pub fn print_dataset_summary(stands: &[Stand], species_summary: &[SpeciesSummary]) {
    println!("\n");
    println!("{CYAN}----------{RESET}");
    println!("{CYAN}Yhteenveto{RESET}");
    println!("{CYAN}----------{RESET}");

    let area: f64 = sum(stands.iter().filter_map(|stand| stand.area));
    let volume: f64 = sum(species_summary.iter().map(|row| row.volume));
    let sawlog: f64 = sum(species_summary.iter().map(|row| row.sawlogvolume));
    let pulpwood: f64 = sum(species_summary.iter().map(|row| row.pulpwoodvolume));
    let value: f64 = sum(species_summary.iter().map(|row| row.value_eur));

    println!(
        "Kuvioiden yhteenlaskettu pinta-ala: {CYAN}{} ha{RESET}",
        grouped_precise(area, 2)
    );
    println!(
        "Puuston kokonaismäärä: {CYAN}{} m³{RESET}",
        grouped_precise(volume, 2)
    );
    println!(
        "Metsäkeskuksen tukkimäärä: {} m³",
        grouped_precise(sawlog, 2)
    );
    println!(
        "Metsäkeskuksen kuitupuumäärä: {} m³",
        grouped_precise(pulpwood, 2)
    );
    println!("LASKENNALLINEN ARVO: {GREEN}{} €{RESET}\n", grouped(value));
}

fn grouped_precise(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    let (integer, fraction) = match text.split_once('.') {
        Some((integer, fraction)) => (integer.to_string(), Some(fraction.to_string())),
        None => (text, None),
    };

    let (sign, digits) = if let Some(rest) = integer.strip_prefix('-') {
        ("-", rest.to_string())
    } else {
        ("", integer)
    };

    let chars: Vec<char> = digits.chars().collect();
    let mut grouped = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if index > 0 && (chars.len() - index) % 3 == 0 {
            grouped.push(' ');
        }
        grouped.push(*ch);
    }

    match fraction {
        Some(fraction) => format!("{sign}{grouped},{fraction}"),
        None => format!("{sign}{grouped}"),
    }
}
