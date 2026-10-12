use crate::consts::{Cutting, Species, TimberType};

/// Kiinteistöön leikattu metsäkuvio (Metsäkeskuksen WFS:stä).
#[derive(Clone, Debug, Default)]
pub struct Stand {
    pub standid: Option<i64>,
    pub standnumber: Option<i64>,
    pub cuttingtype: Option<i64>,
    pub cuttingproposalyear: Option<i64>,
    pub area: Option<f64>,
    #[allow(dead_code)]
    pub intersection_area_m2: f64,
    #[allow(dead_code)]
    pub intersection_area_ha: f64,
}

impl Stand {
    pub fn cutting(&self) -> Option<Cutting> {
        self.cuttingtype.and_then(Cutting::from_code)
    }
}

/// Yhden puulajin tiedot kuviolta (treestratum-taulu).
#[derive(Clone, Debug)]
pub struct TreeStratum {
    pub standid: i64,
    pub treespecies: i64,
    pub age: Option<f64>,
    pub basalarea: Option<f64>,
    pub stemcount: Option<f64>,
    pub meandiameter: Option<f64>,
    pub meanheight: Option<f64>,
    pub volume: Option<f64>,
    pub sawlogpercent: Option<f64>,
    pub sawlogvolume: Option<f64>,
    pub pulpwoodvolume: Option<f64>,
    pub volumegrowth: Option<f64>,
    pub stembiomass: Option<f64>,
    pub stumpbiomass: Option<f64>,
    pub branchbiomass: Option<f64>,
    pub leafbiomass: Option<f64>,
}

/// Koko kuvion puustoyhteenveto (treestandsummary-taulu).
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct StandSummary {
    pub standid: i64,
    pub meanage: Option<f64>,
    pub basalarea: Option<f64>,
    pub stemcount: Option<f64>,
    pub meandiameter: Option<f64>,
    pub meanheight: Option<f64>,
    pub volume: Option<f64>,
    pub sawlogvolume: Option<f64>,
    pub pulpwoodvolume: Option<f64>,
    pub volumegrowth: Option<f64>,
}

/// Puulajikohtainen yhteenveto kuviolta (species_summary).
#[derive(Clone, Debug)]
pub struct SpeciesSummary {
    pub standid: i64,
    pub standnumber: Option<i64>,
    pub species: Species,
    pub cutting: Option<Cutting>,
    pub cuttingproposalyear: Option<i64>,
    pub age: Option<f64>,
    pub stemcount: f64,
    pub meandiameter: Option<f64>,
    pub meanheight: Option<f64>,
    pub area: Option<f64>,
    pub volume: f64,
    pub sawlogvolume: f64,
    pub pulpwoodvolume: f64,
    pub volumegrowth: f64,
    pub growth_pct: Option<f64>,
    pub value_eur: f64,
}

/// Puutavaralajikohtainen rivi (timber_values).
#[derive(Clone, Debug)]
pub struct TimberValue {
    pub standid: i64,
    pub standnumber: Option<i64>,
    pub species: Species,
    pub cutting: Option<Cutting>,
    pub cuttingproposalyear: Option<i64>,
    pub volume: f64,
    pub age: Option<f64>,
    pub timber_type: TimberType,
}

/// Kuviokohtainen yhteenveto (stand_values).
#[derive(Clone, Debug)]
pub struct StandValue {
    pub standid: i64,
    pub standnumber: Option<i64>,
    pub cutting: Option<Cutting>,
    pub cuttingproposalyear: Option<i64>,
    pub volume: f64,
    pub sawlogvolume: f64,
    pub pulpwoodvolume: f64,
    pub volumegrowth: f64,
    pub value_eur: f64,
    pub meanage: Option<f64>,
    pub stemcount: Option<f64>,
    pub meandiameter: Option<f64>,
    pub meanheight: Option<f64>,
    pub area: Option<f64>,
    pub growth_pct: Option<f64>,
}

/// Raakarivi (df), jossa puuston perustiedot ja kuvion tiedot yhdistettynä.
#[derive(Clone, Debug)]
pub struct RawRow {
    pub standid: i64,
    pub treespecies: i64,
    pub species: Species,
    pub age: Option<f64>,
    pub basalarea: Option<f64>,
    pub stemcount: Option<f64>,
    pub meandiameter: Option<f64>,
    pub meanheight: Option<f64>,
    pub volume: Option<f64>,
    pub sawlogpercent: Option<f64>,
    pub sawlogvolume: Option<f64>,
    pub pulpwoodvolume: Option<f64>,
    pub volumegrowth: Option<f64>,
    pub stembiomass: Option<f64>,
    pub stumpbiomass: Option<f64>,
    pub branchbiomass: Option<f64>,
    pub leafbiomass: Option<f64>,
    pub standnumber: Option<i64>,
    pub cutting: Option<Cutting>,
    pub cuttingproposalyear: Option<i64>,
}

/// Koko aineiston tunnusluvut (Yhteenveto-välilehti).
#[derive(Clone, Debug, Default)]
pub struct Totals {
    pub stand_count: i64,
    pub area_ha: f64,
    pub volume_m3: f64,
    pub sawlog_m3: f64,
    pub pulpwood_m3: f64,
    pub growth_m3_per_year: f64,
    pub value_eur: f64,
}
