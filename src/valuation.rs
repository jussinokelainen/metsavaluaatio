use crate::consts::{COST_RATES, CYAN, GREEN, LIMITS, RED, RESET, YELLOW};
use crate::consts::{Cutting, PRICES, PUULAJIT_VALUEGROWTH, Species, TimberType};
use crate::consts::{FertilityClass, MainGroup, SoilType};
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
            fertility_class: FertilityClass::from_opt_code(
                summary.and_then(|summary| summary.fertility_class),
            ),
            main_group: MainGroup::from_opt_code(summary.and_then(|summary| summary.main_group)),
            soil_type: SoilType::from_opt_code(summary.and_then(|summary| summary.soil_type)),
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

fn calculate_sawlog_pct(sawlog: Option<f64>, pulp: Option<f64>) -> Option<f64> {
    let Some(sawlog) = sawlog else { return None };
    let Some(pulp) = pulp else { return None };

    Some((sawlog / (sawlog + pulp)) * 100.0)
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
            sawlogpercent: calculate_sawlog_pct(stratum.sawlogvolume, stratum.pulpwoodvolume),
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
            fertility_class: FertilityClass::from_opt_code(summary.and_then(|s| s.fertility_class)),
            main_group: MainGroup::from_opt_code(summary.and_then(|s| s.main_group)),
            soil_type: SoilType::from_opt_code(summary.and_then(|s| s.soil_type)),
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
    stand_values: &[StandValue],
    fixed_costs: &f64,
    totals: &Totals,
) -> String {
    let mut summary = String::new();
    summary.push_str(&format!("\n"));
    summary.push_str(&format!(
        "
{CYAN}----------{RESET}
{CYAN}Yhteenveto{RESET}
{CYAN}----------{RESET}"
    ));

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
        grouped_precise(totals.area_ha, 2),
        grouped_precise(totals.volume_m3, 2),
        grouped_precise(totals.sawlog_m3, 2),
        grouped_precise(totals.pulpwood_m3, 2),
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

    #[test]
    fn grouped_precise_without_decimals_omits_the_fraction() {
        assert_eq!(grouped_precise(1234.0, 0), "1 234");
        assert_eq!(grouped_precise(1000.0, 0), "1 000");
    }

    #[test]
    fn grouped_precise_keeps_the_minus_sign_of_negative_values() {
        assert_eq!(grouped_precise(-1234567.0, 0), "-1 234 567");
        assert_eq!(grouped_precise(-1234.5, 2), "-1 234,50");
    }

    #[test]
    fn optional_comparators_handle_missing_values() {
        // "tavallinen" järjestys: puuttuva on pienempi
        assert_eq!(cmp_opt_i64(None, None), Ordering::Equal);
        assert_eq!(cmp_opt_i64(None, Some(1)), Ordering::Less);
        assert_eq!(cmp_opt_i64(Some(1), None), Ordering::Greater);
        assert_eq!(cmp_opt_i64(Some(1), Some(2)), Ordering::Less);
        assert_eq!(cmp_opt_i64(Some(2), Some(1)), Ordering::Greater);

        // "_last"-järjestys: puuttuva tulee viimeiseksi
        assert_eq!(cmp_opt_i64_last(None, None), Ordering::Equal);
        assert_eq!(cmp_opt_i64_last(None, Some(1)), Ordering::Greater);
        assert_eq!(cmp_opt_i64_last(Some(1), None), Ordering::Less);
        assert_eq!(cmp_opt_i64_last(Some(1), Some(1)), Ordering::Equal);

        assert_eq!(cmp_opt_f64(None, None), Ordering::Equal);
        assert_eq!(cmp_opt_f64(None, Some(1.0)), Ordering::Less);
        assert_eq!(cmp_opt_f64(Some(1.0), None), Ordering::Greater);
        assert_eq!(cmp_opt_f64(Some(1.0), Some(2.0)), Ordering::Less);

        assert_eq!(cmp_opt_f64_last(None, None), Ordering::Equal);
        assert_eq!(cmp_opt_f64_last(None, Some(1.0)), Ordering::Greater);
        assert_eq!(cmp_opt_f64_last(Some(1.0), None), Ordering::Less);
        assert_eq!(cmp_opt_f64_last(Some(2.0), Some(2.0)), Ordering::Equal);
    }

    fn stand_value() -> StandValue {
        StandValue {
            area: None,
            cutting: None,
            cuttingproposalyear: None,
            fertility_class: FertilityClass::Muu,
            growth_pct: None,
            main_group: MainGroup::Muu,
            main_species: None,
            meanage: None,
            meandiameter: None,
            meanheight: None,
            pulpwoodvolume: 0.0,
            sawlogvolume: 0.0,
            soil_type: SoilType::Muu,
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

    #[test]
    fn costs_without_a_harvest_proposal_are_zero() {
        assert_eq!(calculate_costs(Some(1.0), 16.0, 10.0, None), 0.0);
    }

    /// Yhdistää puuttuvan hakkuuehdotuksen ja annetun ehdotuksen laskemaan
    /// saman arvon: hakkuuehdotus vain valitaan käsittelykykyysrajojen perusteella.
    fn assert_matches_cutting(summary: &SpeciesSummary, expected: Cutting) {
        let explicit = SpeciesSummary {
            cutting: Some(expected),
            ..summary.clone()
        };
        let inferred = calculate_value(summary);
        let direct = calculate_value(&explicit);
        assert_close(inferred, direct);
        assert!(
            inferred > 0.0,
            "hakkuutyypin mukaan laskettu arvo: {inferred}"
        );
    }

    #[test]
    fn missing_harvest_proposal_uses_manty_limits() {
        // Päätehakkuu: ikä ylittää 90 ja läpimitta ylittää 25-3 = 22 cm
        let final_felling = SpeciesSummary {
            species: Species::Manty,
            cutting: None,
            age: Some(95.0),
            meandiameter: Some(23.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_matches_cutting(&final_felling, Cutting::Uudistus);

        // Harvennus: ikä, läpimitta ja korkeus täyttävät harvennusrajan mutta
        // eivät päätehakkuun rajat
        let thinning = SpeciesSummary {
            species: Species::Manty,
            cutting: None,
            age: Some(40.0),
            meandiameter: Some(20.0),
            meanheight: Some(12.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_matches_cutting(&thinning, Cutting::Harvennus);

        // Molemmat rajat alittava mänty ei tuota arvoa
        let young = SpeciesSummary {
            age: Some(20.0),
            meandiameter: Some(15.0),
            meanheight: Some(10.0),
            ..thinning
        };
        assert_eq!(calculate_value(&young), 0.0);
    }

    #[test]
    fn missing_harvest_proposal_uses_koivu_limits() {
        // Päätehakkuu: ikä ylittää 60 ja läpimitta ylittää 27-3 = 24 cm
        let final_felling = SpeciesSummary {
            species: Species::Koivu,
            cutting: None,
            age: Some(65.0),
            meandiameter: Some(25.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_matches_cutting(&final_felling, Cutting::Uudistus);

        // Harvennus täyttää omat rajansa ilman päätehakkuun rajoja
        let thinning = SpeciesSummary {
            species: Species::Koivu,
            cutting: None,
            age: Some(30.0),
            meandiameter: Some(20.0),
            meanheight: Some(15.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_matches_cutting(&thinning, Cutting::Harvennus);

        // Liian matala koivu ei täytä harvennusrajoja
        let too_short = SpeciesSummary {
            meanheight: Some(10.0),
            ..thinning
        };
        assert_eq!(calculate_value(&too_short), 0.0);
    }

    #[test]
    fn stand_value_growth_pct_is_none_without_volume() {
        let without_volume = SpeciesSummary {
            volume: 0.0,
            volumegrowth: 5.0,
            ..row()
        };
        let with_volume = SpeciesSummary {
            standid: 2,
            volume: 100.0,
            volumegrowth: 10.0,
            ..row()
        };

        let values = build_stand_values(&[without_volume, with_volume], &[], &[stand(1), stand(2)]);

        let none_pct = values
            .iter()
            .find(|value| value.standid == 1)
            .expect("kuvio 1 löytyi");
        assert_eq!(none_pct.growth_pct, None);

        let some_pct = values
            .iter()
            .find(|value| value.standid == 2)
            .expect("kuvio 2 löytyi");
        assert_eq!(some_pct.growth_pct, Some(10.0));
    }

    // -----------------------------------------------------------------------
    // Apu-funktiot testidatalle
    // -----------------------------------------------------------------------

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    fn stand(standid: i64) -> Stand {
        Stand {
            standid: Some(standid),
            ..Stand::default()
        }
    }

    fn stand_summary(standid: i64) -> StandSummary {
        StandSummary {
            standid,
            meanage: None,
            basalarea: None,
            stemcount: None,
            meandiameter: None,
            meanheight: None,
            volume: None,
            sawlogvolume: None,
            pulpwoodvolume: None,
            volumegrowth: None,
            main_species: None,
            fertility_class: None,
            main_group: None,
            soil_type: None,
        }
    }

    /// Kuvio jossa on kaikki puustotiedot täytettynä
    fn filled_stratum(standid: i64, treespecies: i64) -> TreeStratum {
        TreeStratum {
            standid,
            treespecies,
            age: Some(50.0),
            basalarea: Some(25.0),
            stemcount: Some(1000.0),
            meandiameter: Some(20.0),
            meanheight: Some(15.0),
            volume: Some(100.0),
            sawlogpercent: Some(40.0),
            sawlogvolume: Some(40.0),
            pulpwoodvolume: Some(60.0),
            volumegrowth: Some(10.0),
            stembiomass: Some(200.0),
            stumpbiomass: Some(30.0),
            branchbiomass: Some(40.0),
            leafbiomass: Some(10.0),
        }
    }

    // -----------------------------------------------------------------------
    // build_species_summary
    // -----------------------------------------------------------------------

    #[test]
    fn species_summary_merges_strata_of_the_same_stand_species_and_age() {
        let stands = vec![Stand {
            standid: Some(1),
            standnumber: Some(7),
            cuttingtype: Some(5),
            cuttingproposalyear: Some(2030),
            area: Some(2.0),
            ..Stand::default()
        }];

        let first = filled_stratum(1, 2);
        let mut second = filled_stratum(1, 2);
        second.volume = Some(50.0);
        second.sawlogvolume = Some(20.0);
        second.pulpwoodvolume = Some(30.0);
        second.volumegrowth = Some(5.0);
        second.meandiameter = Some(22.0);
        second.meanheight = Some(17.0);
        second.stemcount = Some(800.0);

        let summaries = build_species_summary(&[first.clone(), second], &stands).unwrap();
        assert_eq!(summaries.len(), 1);

        let summary = &summaries[0];
        assert_eq!(summary.standid, 1);
        assert_eq!(summary.standnumber, Some(7));
        assert_eq!(summary.species, Species::Kuusi);
        assert!(matches!(summary.cutting, Some(Cutting::Uudistus)));
        assert_eq!(summary.cuttingproposalyear, Some(2030));
        assert_eq!(summary.age, Some(50.0));
        assert_eq!(summary.area, Some(2.0));
        assert_close(summary.volume, 150.0);
        assert_close(summary.sawlogvolume, 60.0);
        assert_close(summary.pulpwoodvolume, 90.0);
        assert_close(summary.volumegrowth, 15.0);
        assert_close(summary.stemcount, 1800.0);
        assert_close(summary.meandiameter.unwrap(), 21.0);
        assert_close(summary.meanheight.unwrap(), 16.0);
        assert_close(summary.growth_pct.unwrap(), 10.0);
        // Arvo lasketaan hakkuuehdotuksen (uudistus) mukaan
        assert!(summary.value_eur > 0.0);

        // Sama data yhtenä rivinä on identtinen kahden rivin kanssa
        let single = build_species_summary(&[first], &stands).unwrap();
        assert_eq!(single.len(), 1);
        assert_close(single[0].volume, 100.0);
    }

    #[test]
    fn species_summary_ignores_strata_without_a_matching_stand() {
        let stands = vec![stand(1)];
        let summaries = build_species_summary(&[filled_stratum(2, 1)], &stands).unwrap();
        assert!(summaries.is_empty());
    }

    #[test]
    fn species_summary_keeps_strata_with_different_ages_apart() {
        let stands = vec![stand(1)];
        let mut young = filled_stratum(1, 2);
        young.age = Some(40.0);
        let mut old = filled_stratum(1, 2);
        old.age = Some(60.0);

        let summaries = build_species_summary(&[young, old], &stands).unwrap();
        assert_eq!(summaries.len(), 2);
        let ages: Vec<Option<f64>> = summaries.iter().map(|row| row.age).collect();
        assert_eq!(ages, vec![Some(40.0), Some(60.0)]);
    }

    #[test]
    fn species_summary_is_ordered_by_standnumber_then_species() {
        let stands = vec![
            Stand {
                standid: Some(1),
                ..Stand::default()
            },
            Stand {
                standid: Some(2),
                standnumber: Some(3),
                ..Stand::default()
            },
            Stand {
                standid: Some(3),
                standnumber: Some(3),
                ..Stand::default()
            },
        ];

        let strata = vec![
            filled_stratum(1, 29), // Koivu, ei kuvionumeroa -> viimeiseksi
            filled_stratum(2, 1),  // Mänty, kuvio 3
            filled_stratum(3, 1),  // Mänty, kuvio 3
            filled_stratum(3, 2),  // Kuusi, kuvio 3
        ];

        let summaries = build_species_summary(&strata, &stands).unwrap();
        let species: Vec<Species> = summaries.iter().map(|row| row.species).collect();
        assert_eq!(
            species,
            vec![
                Species::Kuusi,
                Species::Manty,
                Species::Manty,
                Species::Koivu
            ]
        );
        let standnumbers: Vec<Option<i64>> = summaries.iter().map(|row| row.standnumber).collect();
        assert_eq!(
            standnumbers,
            vec![Some(3), Some(3), Some(3), None],
            "puuttuva kuvionumero järjestetään viimeiseksi"
        );
    }

    #[test]
    fn species_summary_growth_percent_is_none_without_volume() {
        let stands = vec![stand(1)];
        let strata = vec![filled_stratum(1, 2)];
        let summaries = build_species_summary(&strata, &stands).unwrap();
        // Tässä on tilavuutta, joten prosentti lasketaan
        assert!(summaries[0].growth_pct.is_some());

        let mut empty = filled_stratum(1, 2);
        empty.volume = None;
        empty.sawlogvolume = None;
        empty.pulpwoodvolume = None;
        empty.volumegrowth = None;
        let summaries = build_species_summary(&[empty], &stands).unwrap();
        assert_eq!(summaries[0].growth_pct, None);
    }

    #[test]
    fn species_summary_falls_back_to_growth_percent_without_dimensions() {
        let stands = vec![stand(1)];
        let mut stratum = filled_stratum(1, 2);
        stratum.meandiameter = None;
        stratum.meanheight = None;

        let summaries = build_species_summary(&[stratum], &stands).unwrap();
        let summary = &summaries[0];
        assert!(summary.value_growth_pct.is_some());
        assert_close(
            summary.value_growth_pct.unwrap(),
            summary.growth_pct.unwrap(),
        );
    }

    // -----------------------------------------------------------------------
    // calculate_value
    // -----------------------------------------------------------------------

    #[test]
    fn uudistus_value_is_calculated_from_sawlog_and_pulpwood_prices() {
        let summary = SpeciesSummary {
            cutting: Some(Cutting::Uudistus),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_close(calculate_value(&summary), 1034.925);
    }

    #[test]
    fn harvennus_value_is_calculated_from_prices() {
        let summary = SpeciesSummary {
            cutting: Some(Cutting::Harvennus),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        // Huomaa: myös kuitu lasketaan tukkihinnalla
        assert_close(calculate_value(&summary), 481.81);
    }

    #[test]
    fn missing_harvest_proposal_uses_value_limits() {
        // Kuusi: päätehakkuu kun läpimitta ylittää 27 cm
        let final_felling = SpeciesSummary {
            species: Species::Kuusi,
            cutting: None,
            meandiameter: Some(27.0),
            age: Some(40.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_close(calculate_value(&final_felling), 1075.875);

        // ...tai kun ikä ja läpimitta ylittävät rajat
        let old_tree = SpeciesSummary {
            species: Species::Kuusi,
            cutting: None,
            meandiameter: Some(24.0),
            age: Some(80.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_close(calculate_value(&old_tree), 1075.875);
    }

    #[test]
    fn missing_harvest_proposal_falls_back_to_thinning() {
        let thinning = SpeciesSummary {
            species: Species::Kuusi,
            cutting: None,
            meandiameter: Some(20.0),
            meanheight: Some(15.0),
            age: Some(50.0),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_close(calculate_value(&thinning), 502.11);

        // Liian matala puu ei täytä harvennusrajoja
        let too_short = SpeciesSummary {
            meanheight: Some(10.0),
            ..thinning
        };
        assert_eq!(calculate_value(&too_short), 0.0);
    }

    #[test]
    fn each_species_uses_its_own_price_list() {
        let manty = SpeciesSummary {
            species: Species::Manty,
            cutting: Some(Cutting::Uudistus),
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_close(calculate_value(&manty), 1034.925);

        let kuusi = SpeciesSummary {
            species: Species::Kuusi,
            ..manty.clone()
        };
        assert_close(calculate_value(&kuusi), 1075.875);

        let koivu = SpeciesSummary {
            species: Species::Koivu,
            ..manty
        };
        assert_close(calculate_value(&koivu), 853.95);
    }

    #[test]
    fn unknown_species_is_worthless_without_a_harvest_proposal() {
        let summary = SpeciesSummary {
            species: Species::Muu,
            cutting: None,
            meandiameter: Some(50.0),
            meanheight: Some(30.0),
            age: Some(200.0),
            sawlogvolume: 100.0,
            pulpwoodvolume: 100.0,
            ..row()
        };
        assert_eq!(calculate_value(&summary), 0.0);
    }

    #[test]
    fn missing_area_means_no_value() {
        let summary = SpeciesSummary {
            cutting: Some(Cutting::Uudistus),
            area: None,
            sawlogvolume: 10.0,
            pulpwoodvolume: 20.0,
            ..row()
        };
        assert_eq!(calculate_value(&summary), 0.0);
    }

    // -----------------------------------------------------------------------
    // tree_net_gain / calculate_value_growth
    // -----------------------------------------------------------------------

    #[test]
    fn tree_net_gain_needs_both_dimensions() {
        assert_eq!(
            tree_net_gain(
                Species::Kuusi,
                None,
                Some(15.0),
                1000.0,
                40.0,
                60.0,
                5.0,
                None
            ),
            None
        );
        assert_eq!(
            tree_net_gain(
                Species::Kuusi,
                Some(20.0),
                None,
                1000.0,
                40.0,
                60.0,
                5.0,
                None
            ),
            None
        );
        assert_eq!(
            tree_net_gain(
                Species::Muu,
                Some(20.0),
                Some(15.0),
                1000.0,
                40.0,
                60.0,
                5.0,
                None
            ),
            None
        );
    }

    #[test]
    fn tree_net_gain_is_zero_without_volumegrowth() {
        let gain = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            40.0,
            60.0,
            0.0,
            None,
        );
        assert_close(gain.unwrap(), 0.0);
    }

    #[test]
    fn tree_net_gain_grows_with_volumegrowth() {
        let small = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            40.0,
            60.0,
            5.0,
            None,
        )
        .unwrap();
        let large = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            40.0,
            60.0,
            20.0,
            None,
        )
        .unwrap();

        assert!(small > 0.0, "kasvava puu tuottaa arvokasvua: {small}");
        assert!(large > small, "{large} pitäisi olla suurempi kuin {small}");
    }

    #[test]
    fn tree_net_gain_uses_the_given_q_instead_of_the_default() {
        let default_q = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            40.0,
            60.0,
            10.0,
            None,
        )
        .unwrap();
        let custom_q = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            40.0,
            60.0,
            10.0,
            Some(1.0),
        )
        .unwrap();
        assert_ne!(default_q, custom_q);
    }

    #[test]
    fn tree_net_gain_ignores_height_below_twelve_metres() {
        // Alle 12 m korkeuskorjauskerroin on 0, jolloin tulos riippuu vain
        // kasvuprosentista
        let short = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(10.0),
            1000.0,
            40.0,
            60.0,
            5.0,
            None,
        )
        .unwrap();
        assert!(short > 0.0, "tulos: {short}");

        let no_growth = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(10.0),
            1000.0,
            40.0,
            60.0,
            0.0,
            None,
        )
        .unwrap();
        assert_close(no_growth, 0.0);
    }

    #[test]
    fn tree_net_gain_keeps_a_tiny_sawlog_share_unchanged() {
        // t0 = 0.1 / 100.1 < 0.005, jolloin tulosarvo t1 = t0 eikä kasvua
        // skaalata korkeudella
        let gain = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            0.1,
            100.0,
            5.0,
            None,
        )
        .unwrap();
        // Kun t1 == t0, nettokasvu on täsmälleen kasvusuhde g
        assert_close(gain, 5.0 / 100.1);
    }

    #[test]
    fn value_growth_is_a_percentage_of_the_net_gain() {
        let mut summary = row();
        summary.species = Species::Kuusi;
        summary.volume = 100.0;
        summary.sawlogvolume = 40.0;
        summary.pulpwoodvolume = 60.0;
        summary.volumegrowth = 10.0;
        summary.meandiameter = Some(20.0);
        summary.meanheight = Some(15.0);
        summary.stemcount = 1000.0;

        let growth = calculate_value_growth(&summary).unwrap();
        assert!(growth.is_finite());
        assert!(growth > 0.0);

        let direct = tree_net_gain(
            Species::Kuusi,
            Some(20.0),
            Some(15.0),
            1000.0,
            40.0,
            60.0,
            10.0,
            None,
        )
        .unwrap();
        assert_close(growth, direct * 100.0);
    }

    #[test]
    fn value_growth_is_none_without_dimensions() {
        let mut summary = row();
        summary.species = Species::Kuusi;
        summary.volume = 100.0;
        summary.sawlogvolume = 40.0;
        summary.pulpwoodvolume = 60.0;
        summary.volumegrowth = 10.0;
        summary.meandiameter = None;
        summary.meanheight = None;

        assert_eq!(calculate_value_growth(&summary), None);
    }

    // -----------------------------------------------------------------------
    // build_timber_values
    // -----------------------------------------------------------------------

    #[test]
    fn timber_values_have_one_row_per_species_and_timber_type() {
        let mut first = row();
        first.standid = 1;
        first.sawlogvolume = 40.0;
        first.pulpwoodvolume = 60.0;
        let mut second = row();
        second.standid = 2;
        second.species = Species::Kuusi;
        second.sawlogvolume = 10.0;
        second.pulpwoodvolume = 30.0;

        let values = build_timber_values(&[first, second]);
        assert_eq!(values.len(), 4);

        // Ensin kaikki tukkirivit, sitten kuiturivit
        assert!(
            values[..2]
                .iter()
                .all(|row| row.timber_type == TimberType::Tukki)
        );
        assert_close(values[0].volume, 40.0);
        assert_close(values[1].volume, 10.0);

        assert!(
            values[2..]
                .iter()
                .all(|row| row.timber_type == TimberType::Kuitu)
        );
        assert_close(values[2].volume, 60.0);
        assert_close(values[3].volume, 30.0);

        assert_eq!(values[0].species, Species::Manty);
        assert_eq!(values[1].species, Species::Kuusi);
        assert_eq!(values[2].species, Species::Manty);
        assert_eq!(values[3].species, Species::Kuusi);
    }

    #[test]
    fn timber_values_keep_stand_info() {
        let mut summary = row();
        summary.standid = 42;
        summary.standnumber = Some(7);
        summary.cutting = Some(Cutting::Harvennus);
        summary.cuttingproposalyear = Some(2031);
        summary.age = Some(55.0);

        let values = build_timber_values(&[summary]);
        for value in values {
            assert_eq!(value.standid, 42);
            assert_eq!(value.standnumber, Some(7));
            assert!(matches!(value.cutting, Some(Cutting::Harvennus)));
            assert_eq!(value.cuttingproposalyear, Some(2031));
            assert_eq!(value.age, Some(55.0));
        }
    }

    // -----------------------------------------------------------------------
    // build_stand_values
    // -----------------------------------------------------------------------

    fn species_row(standid: i64, species: Species) -> SpeciesSummary {
        SpeciesSummary {
            standid,
            standnumber: Some(7),
            species,
            cutting: Some(Cutting::Uudistus),
            cuttingproposalyear: Some(2030),
            age: Some(50.0),
            stemcount: 1000.0,
            meandiameter: Some(20.0),
            meanheight: Some(15.0),
            area: Some(2.0),
            volume: 100.0,
            sawlogvolume: 40.0,
            pulpwoodvolume: 60.0,
            volumegrowth: 10.0,
            growth_pct: Some(10.0),
            value_growth_pct: Some(12.0),
            value_eur: 500.0,
        }
    }

    #[test]
    fn stand_values_aggregate_species_rows_of_one_stand() {
        let mut kuusi = species_row(1, Species::Kuusi);
        kuusi.sawlogvolume = 40.0;
        kuusi.pulpwoodvolume = 60.0;
        kuusi.volume = 100.0;
        kuusi.volumegrowth = 10.0;
        kuusi.value_eur = 500.0;

        let mut manty = species_row(1, Species::Manty);
        manty.sawlogvolume = 20.0;
        manty.pulpwoodvolume = 30.0;
        manty.volume = 50.0;
        manty.volumegrowth = 5.0;
        manty.value_eur = 300.0;

        let stands = vec![Stand {
            standid: Some(1),
            standnumber: Some(7),
            cuttingtype: Some(5),
            cuttingproposalyear: Some(2030),
            area: Some(2.0),
            ..Stand::default()
        }];

        let mut summary = stand_summary(1);
        summary.meanage = Some(55.0);
        summary.stemcount = Some(1200.0);
        summary.meandiameter = Some(22.0);
        summary.meanheight = Some(16.0);
        summary.main_species = Some(2);
        summary.fertility_class = Some(1);
        summary.main_group = Some(2);
        summary.soil_type = Some(24);

        let values = build_stand_values(&[kuusi, manty], &[summary], &stands);
        assert_eq!(values.len(), 1);

        let value = &values[0];
        assert_eq!(value.standid, 1);
        assert_eq!(value.standnumber, Some(7));
        assert!(matches!(value.cutting, Some(Cutting::Uudistus)));
        assert_eq!(value.cuttingproposalyear, Some(2030));
        assert_close(value.volume, 150.0);
        assert_close(value.sawlogvolume, 60.0);
        assert_close(value.pulpwoodvolume, 90.0);
        assert_close(value.volumegrowth, 15.0);
        assert_close(value.growth_pct.unwrap(), 10.0);
        assert_eq!(value.area, Some(2.0));
        assert_close(value.income_eur, 800.0);

        // Kulut: 2 ha * (0.75*(90+60)) m³ * 0.5 €/m³ + 2 ha * (720 + 1120) €/ha
        assert_close(value.costs_eur, 3792.5);
        assert_close(value.net_eur, 800.0 - 3792.5);

        // Kuvion perustiedot tulevat StandSummarysta
        assert_eq!(value.meanage, Some(55.0));
        assert_eq!(value.stemcount, Some(1200.0));
        assert_eq!(value.meandiameter, Some(22.0));
        assert_eq!(value.meanheight, Some(16.0));
        assert_eq!(value.main_species, Some(2));
        assert!(matches!(value.fertility_class, FertilityClass::Lehto));
        assert!(matches!(value.main_group, MainGroup::KituMaa));
        assert!(matches!(value.soil_type, SoilType::Savimaa));
    }

    #[test]
    fn stand_values_default_to_muu_without_stand_summary() {
        let row = species_row(1, Species::Kuusi);
        let stands = vec![Stand {
            standid: Some(1),
            standnumber: Some(7),
            cuttingtype: Some(5),
            area: Some(2.0),
            ..Stand::default()
        }];

        let values = build_stand_values(&[row], &[], &stands);
        assert_eq!(values.len(), 1);
        let value = &values[0];
        assert_eq!(value.meanage, None);
        assert!(matches!(value.fertility_class, FertilityClass::Muu));
        assert!(matches!(value.main_group, MainGroup::Muu));
        assert!(matches!(value.soil_type, SoilType::Muu));
    }

    #[test]
    fn stand_values_without_area_have_no_costs() {
        let row = species_row(1, Species::Kuusi);
        let stands = vec![Stand {
            standid: Some(1),
            standnumber: Some(7),
            cuttingtype: Some(5),
            ..Stand::default()
        }];

        let values = build_stand_values(&[row], &[], &stands);
        assert_eq!(values[0].area, None);
        assert_close(values[0].costs_eur, 0.0);
        assert_close(values[0].net_eur, values[0].income_eur);
    }

    #[test]
    fn stand_values_are_ordered_by_standnumber_with_missing_last() {
        let mut numbered = species_row(2, Species::Kuusi);
        numbered.standnumber = Some(5);
        let mut unnumbered = species_row(1, Species::Manty);
        unnumbered.standnumber = None;

        let stands = vec![
            Stand {
                standid: Some(1),
                ..Stand::default()
            },
            Stand {
                standid: Some(2),
                standnumber: Some(5),
                ..Stand::default()
            },
        ];

        let values = build_stand_values(&[unnumbered, numbered], &[], &stands);
        let standnumbers: Vec<Option<i64>> = values.iter().map(|row| row.standnumber).collect();
        assert_eq!(standnumbers, vec![Some(5), None]);
    }

    #[test]
    fn stand_values_split_rows_with_different_harvest_proposals() {
        let mut first = species_row(1, Species::Kuusi);
        first.cuttingproposalyear = Some(2030);
        let mut second = species_row(1, Species::Manty);
        second.cuttingproposalyear = Some(2031);

        let stands = vec![Stand {
            standid: Some(1),
            standnumber: Some(7),
            cuttingtype: Some(5),
            area: Some(2.0),
            ..Stand::default()
        }];

        let values = build_stand_values(&[first, second], &[], &stands);
        assert_eq!(values.len(), 2);
        let years: Vec<Option<i64>> = values.iter().map(|row| row.cuttingproposalyear).collect();
        assert_eq!(years, vec![Some(2030), Some(2031)]);
    }

    // -----------------------------------------------------------------------
    // build_raw_rows
    // -----------------------------------------------------------------------

    #[test]
    fn raw_rows_join_strata_stand_and_summary() {
        let strata = vec![filled_stratum(1, 2)];
        let stands = vec![Stand {
            standid: Some(1),
            standnumber: Some(7),
            cuttingtype: Some(3),
            cuttingproposalyear: Some(2032),
            area: Some(2.0),
            ..Stand::default()
        }];
        let mut summary = stand_summary(1);
        summary.fertility_class = Some(1);
        summary.main_group = Some(3);
        summary.soil_type = Some(60);

        let rows = build_raw_rows(&strata, &stands, &[summary]).unwrap();
        assert_eq!(rows.len(), 1);

        let row = &rows[0];
        assert_eq!(row.standid, 1);
        assert_eq!(row.treespecies, 2);
        assert_eq!(row.species, Species::Kuusi);
        assert_eq!(row.standnumber, Some(7));
        assert!(matches!(row.cutting, Some(Cutting::Harvennus)));
        assert_eq!(row.cuttingproposalyear, Some(2032));
        assert_eq!(row.age, Some(50.0));
        assert_eq!(row.basalarea, Some(25.0));
        assert_eq!(row.volume, Some(100.0));
        assert_eq!(row.stembiomass, Some(200.0));
        assert_eq!(row.leafbiomass, Some(10.0));
        assert!(matches!(row.fertility_class, FertilityClass::Lehto));
        assert!(matches!(row.main_group, MainGroup::Joutomaa));
        assert!(matches!(row.soil_type, SoilType::Turvemaa));
    }

    #[test]
    fn raw_rows_calculate_sawlog_percent_from_volumes() {
        let mut stratum = filled_stratum(1, 2);
        stratum.sawlogvolume = Some(30.0);
        stratum.pulpwoodvolume = Some(70.0);

        let rows = build_raw_rows(&[stratum], &[stand(1)], &[]).unwrap();
        assert_close(rows[0].sawlogpercent.unwrap(), 30.0);

        let mut pulp_only = filled_stratum(1, 2);
        pulp_only.sawlogvolume = Some(0.0);
        pulp_only.pulpwoodvolume = Some(50.0);
        let rows = build_raw_rows(&[pulp_only], &[stand(1)], &[]).unwrap();
        assert_close(rows[0].sawlogpercent.unwrap(), 0.0);
    }

    #[test]
    fn raw_rows_are_none_when_volumes_are_missing() {
        let mut stratum = filled_stratum(1, 2);
        stratum.sawlogvolume = None;
        stratum.pulpwoodvolume = None;

        let rows = build_raw_rows(&[stratum], &[stand(1)], &[]).unwrap();
        assert_eq!(rows[0].sawlogpercent, None);
    }

    #[test]
    fn raw_rows_skip_strata_of_unknown_stands() {
        let rows = build_raw_rows(&[filled_stratum(99, 1)], &[stand(1)], &[]).unwrap();
        assert!(rows.is_empty());
    }

    // -----------------------------------------------------------------------
    // compute_totals
    // -----------------------------------------------------------------------

    #[test]
    fn totals_count_each_stand_once_and_scale_by_area() {
        let stands = vec![
            Stand {
                standid: Some(1),
                area: Some(2.0),
                ..Stand::default()
            },
            Stand {
                standid: Some(1),
                area: Some(2.0),
                ..Stand::default()
            },
            Stand {
                standid: Some(2),
                area: Some(3.0),
                ..Stand::default()
            },
            Stand::default(),
        ];

        let mut value = stand_value();
        value.standid = 1;
        value.area = Some(2.0);
        value.volume = 100.0;
        value.sawlogvolume = 40.0;
        value.pulpwoodvolume = 60.0;
        value.volumegrowth = 10.0;
        value.income_eur = 500.0;
        value.costs_eur = 100.0;
        value.net_eur = 400.0;

        let totals = compute_totals(&stands, &[value.clone(), value]);
        assert_eq!(
            totals.stand_count, 2,
            "kuvio lasketaan kerran riippumatta riveistä"
        );
        assert_close(totals.area_ha, 7.0);
        assert_close(totals.volume_m3, 400.0);
        assert_close(totals.sawlog_m3, 160.0);
        assert_close(totals.pulpwood_m3, 240.0);
        assert_close(totals.growth_m3_per_year, 40.0);
        assert_close(totals.income_eur, 1000.0);
        assert_close(totals.costs_eur, 200.0);
        assert_close(totals.net_eur, 800.0);
    }

    #[test]
    fn totals_of_empty_dataset_are_zero() {
        let totals = compute_totals(&[], &[]);
        assert_eq!(totals.stand_count, 0);
        assert_eq!(totals.area_ha, 0.0);
        assert_eq!(totals.net_eur, 0.0);
    }

    // -----------------------------------------------------------------------
    // create_dataset_summary
    // -----------------------------------------------------------------------

    #[test]
    fn dataset_summary_contains_the_key_figures() {
        let mut value = stand_value();
        value.income_eur = 1000.0;
        value.costs_eur = 200.0;
        value.net_eur = 800.0;

        let totals = Totals {
            stand_count: 1,
            area_ha: 1.5,
            volume_m3: 100.0,
            sawlog_m3: 40.0,
            pulpwood_m3: 60.0,
            growth_m3_per_year: 10.0,
            income_eur: 1000.0,
            costs_eur: 200.0,
            net_eur: 800.0,
        };

        let summary = create_dataset_summary(&[value], &109.0, &totals);
        assert!(summary.contains("Yhteenveto"));
        assert!(summary.contains("1,50 ha"));
        assert!(summary.contains("100,00 m³"));
        assert!(summary.contains("40,00 m³"));
        assert!(summary.contains("60,00 m³"));
        assert!(summary.contains("1000.00 €"));
        assert!(summary.contains("200.00 €"));
        assert!(summary.contains("109.00 €"));
        // 800 - 109 = 691, positiivinen arvo värjätään vihreäksi
        assert!(summary.contains("691"));
        assert!(summary.contains(GREEN));
        assert!(!summary.contains(RED));
    }

    #[test]
    fn dataset_summary_marks_a_negative_value_red() {
        let mut value = stand_value();
        value.net_eur = -1000.0;

        let totals = Totals {
            stand_count: 1,
            ..Totals::default()
        };

        let summary = create_dataset_summary(&[value], &0.0, &totals);
        assert!(summary.contains(RED));
        assert!(!summary.contains(GREEN));
        assert!(summary.contains("-1 000"));
    }
}
