use crate::consts::{COST_RATES, CYAN, GREEN, LIMITS, RED, RESET, YELLOW};
use crate::consts::{Cutting, PRICES, PUULAJIT_VALUEGROWTH, Species, TimberType};
use crate::model::{
    RawRow, SpeciesSummary, Stand, StandSummary, StandValue, TimberValue, Totals, TreeStratum,
};
use color_eyre::Result;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};

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
            value_growth_pct: Some(0.0),
            value_eur: 0.0,
        };

        // Yrittää laskea Arvokasvun, käyttää normaalia kasvuprosenttia jos
        // arvokasvun laskeminen epäonnistuu
        match calculate_value_growth(&summary) {
            Some(pct) => summary.value_growth_pct = Some(pct),
            None => summary.value_growth_pct = growth_pct,
        }

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
// Kuviokohtaiset Kulut
// ---------------------------------------------------------------------------

/// Laskee kulut yhdelle kuviolle
pub fn calculate_costs(
    area: Option<f64>,
    sawlog: f64,
    pulpwood: f64,
    cutting: Option<Cutting>,
) -> f64 {
    if let Some(a) = area {
        match cutting {
            Some(Cutting::Ensiharvennus) => 0.0,
            Some(Cutting::Harvennus) => {
                let volume = a * pulpwood * 0.3 + a * sawlog * 0.1;
                let mut cost = volume * COST_RATES.palvelumaksu;
                cost += a * COST_RATES.ennakkoraivaus_harvennus;
                cost
            }
            Some(Cutting::Uudistus) => {
                let volume = a * pulpwood * 0.75 + a * sawlog * 0.75;
                let mut cost = volume * COST_RATES.palvelumaksu;
                cost += a * COST_RATES.maanmuokkaus_uudistus;
                cost += a * COST_RATES.taimet_siementen_uudistus;
                cost
            }
            None => 0.0,
        }
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Arvon laskenta
// ---------------------------------------------------------------------------

fn ge(value: Option<f64>, threshold: f64) -> bool {
    value.map(|value| value >= threshold).unwrap_or(false)
}

fn uudistus(species: Species, sawlog: f64, pulpwood: f64, area: f64) -> f64 {
    // // -15 % ja 10 % yks. kokonaisvolyymista tukista kuiduksi
    // let adjusted_sawlog = area * sawlog * 0.85 - area * volume * 0.1;
    // let adjusted_pulpwood = area * pulpwood * 0.85 + area * volume * 0.1;
    //
    // adjusted_sawlog * PRICES.get(species, TimberType::Tukki, Cutting::Uudistus)
    //     + adjusted_pulpwood * PRICES.get(species, TimberType::Kuitu, Cutting::Uudistus)
    let sawlog_value =
        area * sawlog * 0.75 * PRICES.get(species, TimberType::Tukki, Cutting::Uudistus);
    let pulp_value =
        area * pulpwood * 0.75 * PRICES.get(species, TimberType::Kuitu, Cutting::Uudistus);
    sawlog_value + pulp_value
}

fn harvennus(species: Species, sawlog: f64, pulpwood: f64, area: f64) -> f64 {
    // area * volume * 0.25 * PRICES.get(species, TimberType::Kuitu, Cutting::Harvennus)
    //     + area * sawlog * 0.15 * PRICES.get(species, TimberType::Tukki, Cutting::Harvennus)
    let sawlog_value =
        area * pulpwood * 0.3 * PRICES.get(species, TimberType::Tukki, Cutting::Harvennus);
    let pulp_value =
        area * sawlog * 0.1 * PRICES.get(species, TimberType::Tukki, Cutting::Harvennus);
    sawlog_value + pulp_value
}

/// diameter=cm, height=cm, stemcount=runkoa/ha, v_* m³/ha.
/// Palauttaa None jos diameter tai height on None, tai puulaji Muu.
pub fn tree_net_gain(
    puulaji: Species,
    diameter: Option<f64>,
    height: Option<f64>,
    stemcount: f64,
    v_sawlog: f64,
    v_pulpwood: f64,
    v_growth: f64,
    q: Option<f64>,
) -> Option<f64> {
    // Ennen laskuja tarkistaa funktion ottavat Option parametrit
    let Some(height) = height else {
        return None;
    };
    let Some(diameter) = diameter else {
        return None;
    };

    // Valitsee oikean puulajin Tmax, k, Gtav ja q arvot
    let (tmax, k, gtav, q_default) = match puulaji {
        Species::Kuusi => {
            let species = PUULAJIT_VALUEGROWTH.kuusi;
            (species.tmax, species.k, species.gtav, species.q)
        }
        Species::Koivu => {
            let species = PUULAJIT_VALUEGROWTH.koivu;
            (species.tmax, species.k, species.gtav, species.q)
        }
        Species::Manty => {
            let species = PUULAJIT_VALUEGROWTH.manty;
            (species.tmax, species.k, species.gtav, species.q)
        }
        Species::Muu => return None,
    };

    // Jos funktion kutsuja antoi q arvon, käyttää sitä
    let q = match q {
        Some(q) => q,
        None => q_default,
    };

    let t0 = v_sawlog / (v_sawlog + v_pulpwood);
    let g = v_growth / (v_sawlog + v_pulpwood);
    let s = 100.0 * height / diameter;
    let g_upper = 0.0000785 * stemcount * diameter.powi(2);
    let beta = 0.95_f64.min(0.55_f64.max(1.35 - 0.0075 * s - 0.010 * (g_upper - gtav)));
    let i = beta * g * diameter / 2.0;
    let fh = if height < 12.0 {
        0.0
    } else if height < 17.0 {
        1.15
    } else {
        1.0
    };
    let t1 = if t0 < 0.005 {
        t0
    } else {
        tmax.min(t0 + fh * k * i * t0 * (1.0 - t0 / tmax))
    };
    let value0 = 1.0 + (q - 1.0) * t0;
    let value1 = (1.0 + g) * (1.0 + (q - 1.0) * t1);

    Some(value1 / value0 - 1.0)
}

/// Laskee puulajikohtaisen arvokasvun (€) kuvion tietojen perusteella.
/// Palauttaa None jos diameter tai height on None, tai puulaji Muu.
pub fn calculate_value_growth(row: &SpeciesSummary) -> Option<f64> {
    let total_volume = row.volume;
    let sawlog = row.sawlogvolume;
    let pulpwood = row.pulpwoodvolume;
    let sawlog_percentage = sawlog / (pulpwood + sawlog);
    let sawlog_corr = sawlog_percentage * total_volume;
    let pulpwood_corr = (1.0 - sawlog_percentage) * total_volume;

    match tree_net_gain(
        row.species,
        row.meandiameter,
        row.meanheight,
        row.stemcount,
        sawlog_corr,
        pulpwood_corr,
        row.volumegrowth,
        None,
    ) {
        Some(net) => Some(100.0 * net),
        None => None,
    }
}

/// Laskee puulajikohtaisen arvon (€) kuvion tietojen perusteella.
pub fn calculate_value(row: &SpeciesSummary) -> f64 {
    let species = row.species;
    let cutting = row.cutting;
    let sawlog = row.sawlogvolume;
    let pulpwood = row.pulpwoodvolume;
    let age = row.age;
    let diameter = row.meandiameter;
    let height = row.meanheight;
    let area = row.area.unwrap_or(0.0);

    match cutting {
        Some(Cutting::Ensiharvennus) => return 0.0,
        Some(Cutting::Harvennus) => return harvennus(species, sawlog, pulpwood, area),
        Some(Cutting::Uudistus) => return uudistus(species, sawlog, pulpwood, area),
        None => {}
    }

    // Puuttuva hakkuuehdotus
    match species {
        Species::Kuusi => {
            // Päätehakkuu hintalaskenta
            if ge(diameter, LIMITS.uudistus.kuusi.diameter as f64)
                || (ge(age, LIMITS.uudistus.kuusi.age as f64)
                    && ge(diameter, (LIMITS.uudistus.kuusi.diameter - 3) as f64))
            {
                return uudistus(species, sawlog, pulpwood, area);
            }

            // Harvennus hintalaskenta
            if ge(age, LIMITS.harvennus.kuusi.diameter as f64)
                && ge(diameter, LIMITS.harvennus.kuusi.diameter as f64)
                && ge(height, LIMITS.harvennus.kuusi.height as f64)
            {
                return harvennus(species, sawlog, pulpwood, area);
            }
        }
        Species::Manty => {
            // Päätehakkuu laskenta
            if ge(diameter, LIMITS.uudistus.manty.diameter as f64)
                || (ge(age, LIMITS.uudistus.manty.age as f64)
                    && ge(diameter, (LIMITS.uudistus.manty.diameter - 3) as f64))
            {
                return uudistus(species, sawlog, pulpwood, area);
            }

            // Harvennus hintalaskenta
            if ge(age, LIMITS.harvennus.manty.diameter as f64)
                && ge(diameter, LIMITS.harvennus.manty.diameter as f64)
                && ge(height, LIMITS.harvennus.manty.height as f64)
            {
                return harvennus(species, sawlog, pulpwood, area);
            }
        }
        Species::Koivu => {
            // Päätehakkuu laskenta
            if ge(diameter, LIMITS.uudistus.koivu.diameter as f64)
                || (ge(age, LIMITS.uudistus.koivu.age as f64)
                    && ge(diameter, (LIMITS.uudistus.koivu.diameter - 3) as f64))
            {
                return uudistus(species, sawlog, pulpwood, area);
            }

            // Harvennus hintalaskenta
            if ge(age, LIMITS.harvennus.koivu.diameter as f64)
                && ge(diameter, LIMITS.harvennus.koivu.diameter as f64)
                && ge(height, LIMITS.harvennus.koivu.height as f64)
            {
                return harvennus(species, sawlog, pulpwood, area);
            }
        }
        Species::Muu => {}
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

        let costs_eur = calculate_costs(area, sawlogvolume, pulpwoodvolume, first.cutting);

        values.push(StandValue {
            standid: first.standid,
            standnumber: first.standnumber,
            cutting: first.cutting,
            cuttingproposalyear: first.cuttingproposalyear,
            volume,
            sawlogvolume,
            pulpwoodvolume,
            volumegrowth,
            meanage: summary.and_then(|summary| summary.meanage),
            stemcount: summary.and_then(|summary| summary.stemcount),
            meandiameter: summary.and_then(|summary| summary.meandiameter),
            meanheight: summary.and_then(|summary| summary.meanheight),
            area,
            growth_pct,
            main_species: summary.and_then(|summary| summary.main_species),
            fertility_class: summary.and_then(|summary| summary.fertility_class),
            main_group: summary.and_then(|summary| summary.main_group),
            soil_type: summary.and_then(|summary| summary.soil_type),
            income_eur: value_eur,
            costs_eur,
            net_eur: value_eur - costs_eur,
        });
    }

    values.sort_by(|a, b| cmp_opt_i64_last(a.standnumber, b.standnumber));

    values
}

pub fn build_fixed_costs(stand_values: &[StandValue]) -> f64 {
    let mut seen = HashSet::new();

    let assignments: Vec<Option<i64>> = stand_values
        .iter()
        .filter(|s| s.cutting == Some(Cutting::Uudistus) || s.cutting == Some(Cutting::Harvennus))
        .map(|s| s.cuttingproposalyear)
        .filter(|y| seen.insert(*y))
        .filter(|y| y.is_some())
        .collect();

    assignments.len() as f64 * (COST_RATES.perusmaksu + COST_RATES.hakkuun_tarkastus)
}

/// Rakentaa raakarivit (df): puuston perustiedot + kuvion tiedot.
pub fn build_raw_rows(
    strata: &[TreeStratum],
    stands: &[Stand],
    stand_summaries: &[StandSummary],
) -> Result<Vec<RawRow>> {
    let mut rows = Vec::new();

    for stratum in strata {
        let Some(stand) = stands
            .iter()
            .find(|stand| stand.standid == Some(stratum.standid))
        else {
            continue;
        };

        let summary = stand_summaries
            .iter()
            .find(|summary| summary.standid == stratum.standid);

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
            fertility_class: summary.and_then(|s| s.fertility_class),
            main_group: summary.and_then(|s| s.main_group),
            soil_type: summary.and_then(|s| s.soil_type),
        });
    }

    Ok(rows)
}

/// Laskee koko aineiston tunnusluvut (Excelin Yhteenveto-välilehti).
pub fn compute_totals(stands: &[Stand], species_summary: &[StandValue]) -> Totals {
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
    let income_eur = sum(species_summary.iter().map(|row| row.income_eur));
    let costs_eur = sum(species_summary.iter().map(|row| row.costs_eur));
    let net_eur = sum(species_summary.iter().map(|row| row.net_eur));

    Totals {
        stand_count: stand_ids.len() as i64,
        area_ha,
        volume_m3,
        sawlog_m3,
        pulpwood_m3,
        growth_m3_per_year,
        income_eur,
        costs_eur,
        net_eur,
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
pub fn create_dataset_summary(
    stands: &[Stand],
    stand_values: &[StandValue],
    fixed_costs: &f64,
) -> String {
    let mut summary = String::new();
    summary.push_str(&format!("\n"));
    summary.push_str(&format!(
        "
{CYAN}----------{RESET}
{CYAN}Yhteenveto{RESET}
{CYAN}----------{RESET}"
    ));

    let area: f64 = sum(stands.iter().filter_map(|stand| stand.area));
    let volume: f64 = sum(stand_values.iter().map(|row| row.volume));
    let sawlog: f64 = sum(stand_values.iter().map(|row| row.sawlogvolume));
    let pulpwood: f64 = sum(stand_values.iter().map(|row| row.pulpwoodvolume));

    let value = sum(stand_values.iter().map(|s| s.net_eur)) - fixed_costs;
    summary.push_str(&format!(
        "
Kuvioiden yhteenlaskettu pinta-ala: {CYAN}{} ha{RESET}
Puuston kokonaismäärä: {CYAN}{} m³{RESET}
Metsäkeskuksen tukkimäärä: {} m³
Metsäkeskuksen kuitupuumäärä: {} m³
Tulot: {CYAN}{:.2} €{RESET}
Muuttuvat kulut: {YELLOW}{:.2} €{RESET}
Kiinteät kulut: {YELLOW}{fixed_costs:.2} €{RESET}
LASKENNALLINEN ARVO: {}{} €{RESET}\n",
        grouped_precise(area, 2),
        grouped_precise(volume, 2),
        grouped_precise(sawlog, 2),
        grouped_precise(pulpwood, 2),
        sum(stand_values.iter().map(|s| s.income_eur)),
        sum(stand_values.iter().map(|s| s.costs_eur)),
        if value > 0.0 { GREEN } else { RED },
        grouped(value)
    ));

    summary
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Neutraali rivi jonka arvoja voidaan muuttaa testi kerrallaan
    fn row() -> SpeciesSummary {
        SpeciesSummary {
            standid: 1,
            standnumber: Some(1),
            species: Species::Manty,
            cutting: None,
            cuttingproposalyear: None,
            age: None,
            stemcount: 0.0,
            meandiameter: None,
            meanheight: None,
            area: Some(1.0),
            volume: 0.0,
            sawlogvolume: 0.0,
            pulpwoodvolume: 0.0,
            volumegrowth: 0.0,
            growth_pct: None,
            value_eur: 0.0,
            value_growth_pct: None,
        }
    }

    #[test]
    fn ensiharvennus_is_always_worthless() {
        let summary = SpeciesSummary {
            cutting: Some(Cutting::Ensiharvennus),
            volume: 100.0,
            sawlogvolume: 50.0,
            pulpwoodvolume: 50.0,
            ..row()
        };
        assert_eq!(calculate_value(&summary), 0.0);
    }

    #[test]
    fn young_small_trees_have_no_value() {
        let summary = SpeciesSummary {
            species: Species::Kuusi,
            age: Some(20.0),
            meandiameter: Some(10.0),
            meanheight: Some(10.0),
            volume: 30.0,
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_eq!(calculate_value(&summary), 0.0);
    }

    #[test]
    fn grouped_inserts_spaces_as_thousand_separators() {
        assert_eq!(grouped(999.0), "999");
        assert_eq!(grouped(1000.0), "1 000");
        assert_eq!(grouped(1234567.0), "1 234 567");
        assert_eq!(grouped(-1000.0), "-1 000");
    }

    #[test]
    fn grouped_precise_uses_a_comma_for_decimals() {
        assert_eq!(grouped_precise(1234.5, 2), "1 234,50");
        assert_eq!(grouped_precise(1000.0, 2), "1 000,00");
    }

    fn stand_value() -> StandValue {
        StandValue {
            area: None,
            cutting: None,
            cuttingproposalyear: None,
            fertility_class: None,
            growth_pct: None,
            main_group: None,
            main_species: None,
            meanage: None,
            meandiameter: None,
            meanheight: None,
            pulpwoodvolume: 0.0,
            sawlogvolume: 0.0,
            soil_type: None,
            standid: 0,
            standnumber: None,
            stemcount: None,
            volume: 0.0,
            volumegrowth: 0.0,
            net_eur: 0.0,
            income_eur: 0.0,
            costs_eur: 0.0,
        }
    }

    #[test]
    fn fixed_costs_remove_duplicates() {
        let values = StandValue {
            cutting: Some(Cutting::Harvennus),
            cuttingproposalyear: Some(2020),
            ..stand_value()
        };
        assert_eq!(
            build_fixed_costs(&[values.clone(), values.clone(), values.clone(), values]),
            (COST_RATES.perusmaksu + COST_RATES.hakkuun_tarkastus)
        )
    }

    #[test]
    fn fixed_costs_keep_uniques() {
        let mut values = Vec::new();
        for i in 2000..2050 {
            let value = StandValue {
                cutting: Some(Cutting::Harvennus),
                cuttingproposalyear: Some(i),
                ..stand_value()
            };
            values.push(value);
        }
        assert_eq!(
            build_fixed_costs(&values),
            50.0 * (COST_RATES.perusmaksu + COST_RATES.hakkuun_tarkastus)
        )
    }

    #[test]
    fn costs_ensiharvennus_gives_always_zero() {
        let res = calculate_costs(Some(1.0), 16.0, 10.0, Some(Cutting::Ensiharvennus));
        assert_eq!(res, 0.0)
    }

    #[test]
    fn costs_harvennus_gives_correct_value() {
        for i in 0..500 {
            let a = i as f64;
            let res = calculate_costs(Some(a), 1.0 / 0.1, 1.0 / 0.3, Some(Cutting::Harvennus));
            let want = a * 2.0 * COST_RATES.palvelumaksu + a * COST_RATES.ennakkoraivaus_harvennus;
            assert_eq!(res, want)
        }
    }

    #[test]
    fn costs_uudistus_gives_correct_value() {
        for i in 0..500 {
            let a = i as f64;
            let res = calculate_costs(Some(a), 1.0 / 0.75, 1.0 / 0.75, Some(Cutting::Uudistus));
            let want = a * 2.0 * COST_RATES.palvelumaksu
                + a * COST_RATES.maanmuokkaus_uudistus
                + a * COST_RATES.taimet_siementen_uudistus;
            assert_eq!(res, want)
        }
    }
}
