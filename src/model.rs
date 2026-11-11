use crate::consts::{Cutting, FertilityClass, MainGroup, SoilType, Species, TimberType};

/// Kiinteistöön leikattu metsäkuvio (Metsäkeskuksen WFS:stä).
#[derive(Clone, Debug, Default)]
pub struct Stand {
    pub standid: Option<i64>,
    pub standnumber: Option<i64>,
    pub cuttingtype: Option<i64>,
    pub cuttingproposalyear: Option<i64>,
    pub area: Option<f64>,
    pub intersection_area_m2: f64,
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
#[derive(Clone, Debug, PartialEq)]
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
    pub main_species: Option<i64>,
    pub fertility_class: Option<i64>,
    pub main_group: Option<i64>,
    pub soil_type: Option<i64>,
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
    pub value_growth_pct: Option<f64>,
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
    pub meanage: Option<f64>,
    pub stemcount: Option<f64>,
    pub meandiameter: Option<f64>,
    pub meanheight: Option<f64>,
    pub area: Option<f64>,
    pub growth_pct: Option<f64>,
    pub main_species: Option<i64>,
    pub fertility_class: FertilityClass,
    pub main_group: MainGroup,
    pub soil_type: SoilType,
    pub income_eur: f64,
    pub costs_eur: f64,
    pub net_eur: f64,
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
    pub fertility_class: FertilityClass,
    pub main_group: MainGroup,
    pub soil_type: SoilType,
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
    pub income_eur: f64,
    pub costs_eur: f64,
    pub net_eur: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand_with_cutting(cuttingtype: Option<i64>) -> Stand {
        Stand {
            cuttingtype,
            ..Stand::default()
        }
    }

    #[test]
    fn cutting_is_derived_from_the_cutting_type_code() {
        assert!(matches!(
            stand_with_cutting(Some(5)).cutting(),
            Some(Cutting::Uudistus)
        ));
        assert!(matches!(
            stand_with_cutting(Some(3)).cutting(),
            Some(Cutting::Harvennus)
        ));
        assert!(matches!(
            stand_with_cutting(Some(2)).cutting(),
            Some(Cutting::Ensiharvennus)
        ));
    }

    #[test]
    fn missing_or_unknown_cutting_type_has_no_cutting() {
        assert_eq!(stand_with_cutting(None).cutting(), None);
        assert_eq!(stand_with_cutting(Some(99)).cutting(), None);
    }

    #[test]
    fn totals_default_to_zero() {
        let totals = Totals::default();
        assert_eq!(totals.stand_count, 0);
        assert_eq!(totals.area_ha, 0.0);
        assert_eq!(totals.net_eur, 0.0);
    }
}
