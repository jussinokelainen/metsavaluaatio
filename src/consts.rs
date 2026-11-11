// Värikoodit
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[36m";
pub const RESET: &str = "\x1b[0m";

// MML / Metsäkeskus rajapinnat
pub const MML_BASE_URL: &str =
    "https://avoin-paikkatieto.maanmittauslaitos.fi/kiinteisto-avoin/simple-features/v3";
pub const SMK_STAND_WFS: &str = "https://avoin.metsakeskus.fi/rajapinnat/v2/stand/ows";
#[allow(dead_code)]
pub const SMK_HABITAT_WFS: &str = "https://avoin.metsakeskus.fi/rajapinnat/v2/habitat/ows";
pub const MUNICIPALITY_ZIP_BASE: &str =
    "https://avoin.metsakeskus.fi/aineistot/Metsavarakuviot/Kunta/MV_";

pub const EPSG_WGS84: &str = "EPSG:4326";
pub const EPSG_TM35FIN: &str = "EPSG:3067";

/// Pienin sallittu leikkauspinta-ala (m²), jotta kuvio otetaan mukaan.
pub const MIN_INTERSECTION_AREA_M2: f64 = 1.0;

/// Puulaji.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    Manty,
    Kuusi,
    Koivu,
    Muu,
}

impl Species {
    /// Metsäkeskuksen `treespecies`-koodi -> puulaji.
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::Manty),
            2 => Some(Self::Kuusi),
            29 => Some(Self::Koivu),
            _ => Some(Self::Muu),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Manty => "Mänty",
            Self::Kuusi => "Kuusi",
            Self::Koivu => "Koivu",
            Self::Muu => "Muu",
        }
    }
}

/// Puutavaralaji.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimberType {
    Tukki,
    Kuitu,
}

impl TimberType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Tukki => "tukki",
            Self::Kuitu => "kuitu",
        }
    }
}

/// Hakkuutapa.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cutting {
    Ensiharvennus,
    Harvennus,
    Uudistus,
}

impl Cutting {
    /// Metsäkeskuksen `cuttingtype`-koodi -> hakkuutapa.
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            2 => Some(Self::Ensiharvennus),
            3 => Some(Self::Harvennus),
            5 => Some(Self::Uudistus),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Ensiharvennus => "ensiharvennus",
            Self::Harvennus => "harvennus",
            Self::Uudistus => "uudistus",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SoilType {
    Keskikarkea,
    KarkeaMoreeni,
    KarkeaLajittunut,
    HienojakoinenKangasmaa,
    HienojakoinenMoreeni,
    HienojakoinenLajittunut,
    Silttipitoinen,
    Savimaa,
    KivinenKeskikarkeaKangas,
    KivinenKarkeaMoreeni,
    KivinenKarkeaLajittunut,
    KivinenHieno,
    Turvemaa,
    SaraTurve,
    RahkaTurve,
    PuuvaltainenTurve,
    EroosioherkkäSara,
    EroosioherkkäRahka,
    MaatumatonSara,
    MaatumatonRahka,
    MustikkaKangasI,
    MustikkaKangasII,
    PuolukkaKangasI,
    PuolukkaKangasII,
    RuohoTurveKangas,
    VarpuTurveKangas,
    Muu,
}

impl SoilType {
    pub fn from_opt_code(code: Option<i64>) -> Self {
        if let Some(c) = code {
            match c {
                10 => Self::Keskikarkea,
                11 => Self::KarkeaMoreeni,
                12 => Self::KarkeaLajittunut,
                20 => Self::HienojakoinenKangasmaa,
                21 => Self::HienojakoinenMoreeni,
                22 => Self::HienojakoinenLajittunut,
                23 => Self::Silttipitoinen,
                24 => Self::Savimaa,
                30 => Self::KivinenKeskikarkeaKangas,
                31 => Self::KivinenKarkeaMoreeni,
                32 => Self::KivinenKarkeaLajittunut,
                40 => Self::KivinenHieno,
                60 => Self::Turvemaa,
                61 => Self::SaraTurve,
                62 => Self::RahkaTurve,
                63 => Self::PuuvaltainenTurve,
                64 => Self::EroosioherkkäSara,
                65 => Self::EroosioherkkäRahka,
                66 => Self::MaatumatonSara,
                67 => Self::MaatumatonRahka,
                791 => Self::MustikkaKangasI,
                792 => Self::MustikkaKangasII,
                793 => Self::PuolukkaKangasI,
                794 => Self::PuolukkaKangasII,
                795 => Self::RuohoTurveKangas,
                796 => Self::VarpuTurveKangas,
                _ => Self::Muu,
            }
        } else {
            Self::Muu
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Keskikarkea => "Keskikarkea tai karkea kangasmaa",
            Self::KarkeaMoreeni => "Karkea moreeni",
            Self::KarkeaLajittunut => "Karkea lajittunut maalaji",
            Self::HienojakoinenKangasmaa => "Hienojakoinen kangasmaa",
            Self::HienojakoinenMoreeni => "Hienojakoinen moreeni",
            Self::HienojakoinenLajittunut => "Hienojakoinen lajittunut maalaji",
            Self::Silttipitoinen => "Silttipitoinen maalaji",
            Self::Savimaa => "Savimaa",
            Self::KivinenKeskikarkeaKangas => "Kivinen Keskikarkea tai karkea kangasmaa",
            Self::KivinenKarkeaMoreeni => "Kivinen karkea moreeni",
            Self::KivinenKarkeaLajittunut => "Kivinen karkea lajittunut maalaji",
            Self::KivinenHieno => "Kivinen hienojakoinen kangasmaa",
            Self::Turvemaa => "Turvemaa",
            Self::SaraTurve => "Saraturve",
            Self::RahkaTurve => "Rahkaturve",
            Self::PuuvaltainenTurve => "Puuvaltainen turve",
            Self::EroosioherkkäSara => "Eroosioherkkä saraturve (von Post luokka yli 5)",
            Self::EroosioherkkäRahka => "Eroosioherkkä rahkaturve (von Post luokka yli 5)",
            Self::MaatumatonSara => "Maatumaton saraturve (von Post luokka enintään 5)",
            Self::MaatumatonRahka => "Maatumaton rahkaturve (von Post enintään 5)",
            Self::MustikkaKangasI => "Mustikkaturvekangas (I)",
            Self::MustikkaKangasII => "Mustikkaturvekangas (II)",
            Self::PuolukkaKangasI => "Puolukkaturvekangas (I)",
            Self::PuolukkaKangasII => "Puolukkaturvekangas (II)",
            Self::RuohoTurveKangas => "Ruohoturvekangas",
            Self::VarpuTurveKangas => "Varputurvekangas",
            Self::Muu => "Muu",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MainGroup {
    MetsaMaa,
    KituMaa,
    Joutomaa,
    MuuMetsaTalous,
    Muu,
}

impl MainGroup {
    pub fn from_opt_code(code: Option<i64>) -> Self {
        if let Some(c) = code {
            match c {
                1 => Self::MetsaMaa,
                2 => Self::KituMaa,
                3 => Self::Joutomaa,
                4 => Self::MuuMetsaTalous,
                _ => Self::Muu,
            }
        } else {
            Self::Muu
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::MetsaMaa => "Metsämaa",
            Self::KituMaa => "Kitumaa",
            Self::Joutomaa => "Joutomaa",
            Self::MuuMetsaTalous => "Muu metsätalousmaa",
            Self::Muu => "Muu",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FertilityClass {
    Kalliomaa,
    KangasKarukko,
    KangasKuiva,
    KangasKuivahko,
    KangasKuivahkoTaiKaru,
    KangasLehtomainen,
    KangasTuore,
    KangasTuoreTaiReheva,
    Lakimaa,
    Lehto,
    Muu,
}

impl FertilityClass {
    pub fn from_opt_code(code: Option<i64>) -> Self {
        if let Some(c) = code {
            match c {
                1 => Self::Lehto,
                2 => Self::KangasLehtomainen,
                3 => Self::KangasTuore,
                4 => Self::KangasKuivahko,
                5 => Self::KangasKuiva,
                6 => Self::KangasKarukko,
                7 => Self::Kalliomaa,
                8 => Self::Lakimaa,
                9 => Self::KangasKuivahkoTaiKaru,
                10 => Self::KangasTuoreTaiReheva,
                _ => Self::Muu,
            }
        } else {
            Self::Muu
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Kalliomaa => "Kalliomaa ja hietikko",
            Self::KangasKarukko => "Karukko kangas tai vastaava suo",
            Self::KangasKuiva => "Kuiva kangas tai vastaava suo",
            Self::KangasKuivahko => "Kuivahko kangas tai vastaava suo",
            Self::KangasKuivahkoTaiKaru => "Kuivahko kangas tai sitä karumpi, paperi-ilmoitukset.",
            Self::KangasLehtomainen => "Lehtomainen kangas tai vastaava suo",
            Self::KangasTuore => "Tuore kangas tai vastaava suo",
            Self::KangasTuoreTaiReheva => "Tuore kangas tai sitä rehevämpi, paperi-ilmoitukset.",
            Self::Lakimaa => "Lakimaa ja tunturi",
            Self::Lehto => "Lehto tai vastaava suo",
            Self::Muu => "Muu",
        }
    }
}

struct CuttingPrices {
    uudistus: f64,
    harvennus: f64,
    ensiharvennus: f64,
}

impl CuttingPrices {
    fn get(&self, cutting: Cutting) -> f64 {
        match cutting {
            Cutting::Uudistus => self.uudistus,
            Cutting::Harvennus => self.harvennus,
            Cutting::Ensiharvennus => self.ensiharvennus,
        }
    }
}

struct WoodPrices {
    tukki: CuttingPrices,
    kuitu: CuttingPrices,
}

impl WoodPrices {
    fn get(&self, timber: TimberType, cutting: Cutting) -> f64 {
        match timber {
            TimberType::Tukki => self.tukki.get(cutting),
            TimberType::Kuitu => self.kuitu.get(cutting),
        }
    }
}

/// Hinnasto puulajeittain.
pub struct PriceList {
    manty: WoodPrices,
    kuusi: WoodPrices,
    koivu: WoodPrices,
}

impl PriceList {
    pub fn get(&self, species: Species, timber: TimberType, cutting: Cutting) -> f64 {
        let wood = match species {
            Species::Manty => &self.manty,
            Species::Kuusi => &self.kuusi,
            Species::Koivu => &self.koivu,
            // Muu puulaji, hinta 0
            Species::Muu => return 0.0,
        };
        wood.get(timber, cutting)
    }
}

pub const PRICES: PriceList = PriceList {
    manty: WoodPrices {
        tukki: CuttingPrices {
            uudistus: 80.37,
            harvennus: 68.83,
            ensiharvennus: 57.24,
        },
        kuitu: CuttingPrices {
            uudistus: 28.81,
            harvennus: 21.89,
            ensiharvennus: 15.95,
        },
    },
    kuusi: WoodPrices {
        tukki: CuttingPrices {
            uudistus: 85.01,
            harvennus: 71.73,
            ensiharvennus: 60.19,
        },
        kuitu: CuttingPrices {
            uudistus: 29.22,
            harvennus: 21.75,
            ensiharvennus: 15.95,
        },
    },
    koivu: WoodPrices {
        tukki: CuttingPrices {
            uudistus: 62.94,
            harvennus: 53.70,
            ensiharvennus: 39.74,
        },
        kuitu: CuttingPrices {
            uudistus: 25.46,
            harvennus: 21.71,
            ensiharvennus: 16.62,
        },
    },
};

#[allow(dead_code)]
pub struct StandCosts {
    pub perusmaksu: f64,
    pub hakkuun_tarkastus: f64,
    pub palvelumaksu: f64,
    pub maanmuokkaus_uudistus: f64,
    pub ennakkoraivaus_harvennus: f64,
    pub taimet_siementen_uudistus: f64,
}

/// Perusmaksu: €/toimeksianto
/// Hakkuun_Tarkastus: €/toimeksianto
/// Palvelumaksu: €/m³
/// Maanmuokkaus_Uudistus: €/ha
/// Ennakkoraivaus_Harvennus: €/ha
/// Taimet_Siementen_Uudistus: €/ha
pub const COST_RATES: StandCosts = {
    StandCosts {
        perusmaksu: 603.0,
        hakkuun_tarkastus: 306.0,
        palvelumaksu: 0.5,
        maanmuokkaus_uudistus: 720.0,
        ennakkoraivaus_harvennus: 396.0,
        taimet_siementen_uudistus: 1120.0,
    }
};

pub struct SpeciesValueLimits {
    pub age: i64,
    pub diameter: i64,
    pub height: i64,
}

pub struct CutTypeValueLimits {
    pub kuusi: SpeciesValueLimits,
    pub manty: SpeciesValueLimits,
    pub koivu: SpeciesValueLimits,
}
pub struct ValueLimits {
    pub uudistus: CutTypeValueLimits,
    pub harvennus: CutTypeValueLimits,
}

pub const LIMITS: ValueLimits = ValueLimits {
    uudistus: CutTypeValueLimits {
        kuusi: SpeciesValueLimits {
            age: 80,
            diameter: 27,
            height: 0,
        },
        manty: SpeciesValueLimits {
            age: 90,
            diameter: 25,
            height: 0,
        },
        koivu: SpeciesValueLimits {
            age: 60,
            diameter: 27,
            height: 0,
        },
    },
    harvennus: CutTypeValueLimits {
        kuusi: SpeciesValueLimits {
            age: 35,
            diameter: 12,
            height: 12,
        },
        manty: SpeciesValueLimits {
            age: 30,
            diameter: 11,
            height: 11,
        },
        koivu: SpeciesValueLimits {
            age: 25,
            diameter: 12,
            height: 14,
        },
    },
};

pub struct ValueGrowthSpecies {
    pub tmax: f64,
    pub k: f64,
    pub gtav: f64,
    pub q: f64,
}

pub struct ValueGrowth {
    pub kuusi: ValueGrowthSpecies,
    pub manty: ValueGrowthSpecies,
    pub koivu: ValueGrowthSpecies,
}

/// puulaji: (tmax, k, gtav, q) q = tukin hintasuhde kuituun
pub const PUULAJIT_VALUEGROWTH: ValueGrowth = ValueGrowth {
    kuusi: ValueGrowthSpecies {
        tmax: 0.89,
        k: 0.261,
        gtav: 26.0,
        q: 3.0,
    },
    manty: ValueGrowthSpecies {
        tmax: 0.90,
        k: 0.275,
        gtav: 22.0,
        q: 3.0,
    },
    koivu: ValueGrowthSpecies {
        tmax: 0.72,
        k: 0.220,
        gtav: 20.0,
        q: 2.2,
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_from_code_maps_known_codes() {
        assert_eq!(Species::from_code(1), Some(Species::Manty));
        assert_eq!(Species::from_code(2), Some(Species::Kuusi));
        assert_eq!(Species::from_code(29), Some(Species::Koivu));
    }

    #[test]
    fn species_from_unknown_codes_gives_muu() {
        assert_eq!(Species::from_code(0), Some(Species::Muu));
        assert_eq!(Species::from_code(99), Some(Species::Muu));
    }

    #[test]
    fn cutting_from_code_maps_known_codes() {
        assert_eq!(Cutting::from_code(2), Some(Cutting::Ensiharvennus));
        assert_eq!(Cutting::from_code(3), Some(Cutting::Harvennus));
        assert_eq!(Cutting::from_code(5), Some(Cutting::Uudistus));
    }

    #[test]
    fn cutting_from_code_rejects_unknown_codes() {
        assert_eq!(Cutting::from_code(1), None);
        assert_eq!(Cutting::from_code(4), None);
        assert_eq!(Cutting::from_code(67), None);
    }

    #[test]
    fn price_list_is_species_and_cutting_specific() {
        assert_eq!(
            PRICES.get(Species::Manty, TimberType::Tukki, Cutting::Uudistus),
            80.37
        );
        assert_eq!(
            PRICES.get(Species::Manty, TimberType::Kuitu, Cutting::Harvennus),
            21.89
        );
        assert_eq!(
            PRICES.get(Species::Kuusi, TimberType::Tukki, Cutting::Ensiharvennus),
            60.19
        );
        assert_eq!(
            PRICES.get(Species::Kuusi, TimberType::Kuitu, Cutting::Uudistus),
            29.22
        );
        assert_eq!(
            PRICES.get(Species::Koivu, TimberType::Tukki, Cutting::Harvennus),
            53.70
        );
        assert_eq!(
            PRICES.get(Species::Koivu, TimberType::Kuitu, Cutting::Ensiharvennus),
            16.62
        );
    }

    #[test]
    fn unknown_species_have_no_price() {
        for cutting in [
            Cutting::Uudistus,
            Cutting::Harvennus,
            Cutting::Ensiharvennus,
        ] {
            for timber in [TimberType::Tukki, TimberType::Kuitu] {
                assert_eq!(PRICES.get(Species::Muu, timber, cutting), 0.0);
            }
        }
    }

    #[test]
    fn sawlog_is_more_expensive_than_pulpwood() {
        for species in [Species::Manty, Species::Kuusi, Species::Koivu] {
            for cutting in [
                Cutting::Uudistus,
                Cutting::Harvennus,
                Cutting::Ensiharvennus,
            ] {
                assert!(
                    PRICES.get(species, TimberType::Tukki, cutting)
                        > PRICES.get(species, TimberType::Kuitu, cutting)
                );
            }
        }
    }

    #[test]
    fn soil_type_codes_map_to_their_names() {
        assert_eq!(
            SoilType::from_opt_code(Some(10)).name(),
            "Keskikarkea tai karkea kangasmaa"
        );
        assert_eq!(SoilType::from_opt_code(Some(11)).name(), "Karkea moreeni");
        assert_eq!(SoilType::from_opt_code(Some(24)).name(), "Savimaa");
        assert_eq!(SoilType::from_opt_code(Some(60)).name(), "Turvemaa");
        assert_eq!(
            SoilType::from_opt_code(Some(791)).name(),
            "Mustikkaturvekangas (I)"
        );
        assert_eq!(
            SoilType::from_opt_code(Some(796)).name(),
            "Varputurvekangas"
        );
    }

    #[test]
    fn unknown_soil_type_codes_are_muu() {
        assert_eq!(SoilType::from_opt_code(None).name(), "Muu");
        assert_eq!(SoilType::from_opt_code(Some(0)).name(), "Muu");
        assert_eq!(SoilType::from_opt_code(Some(9999)).name(), "Muu");
    }

    #[test]
    fn every_soil_type_code_is_mapped() {
        for (code, name) in [
            (10, "Keskikarkea tai karkea kangasmaa"),
            (11, "Karkea moreeni"),
            (12, "Karkea lajittunut maalaji"),
            (20, "Hienojakoinen kangasmaa"),
            (21, "Hienojakoinen moreeni"),
            (22, "Hienojakoinen lajittunut maalaji"),
            (23, "Silttipitoinen maalaji"),
            (24, "Savimaa"),
            (30, "Kivinen Keskikarkea tai karkea kangasmaa"),
            (31, "Kivinen karkea moreeni"),
            (32, "Kivinen karkea lajittunut maalaji"),
            (40, "Kivinen hienojakoinen kangasmaa"),
            (60, "Turvemaa"),
            (61, "Saraturve"),
            (62, "Rahkaturve"),
            (63, "Puuvaltainen turve"),
            (64, "Eroosioherkkä saraturve (von Post luokka yli 5)"),
            (65, "Eroosioherkkä rahkaturve (von Post luokka yli 5)"),
            (66, "Maatumaton saraturve (von Post luokka enintään 5)"),
            (67, "Maatumaton rahkaturve (von Post enintään 5)"),
            (791, "Mustikkaturvekangas (I)"),
            (792, "Mustikkaturvekangas (II)"),
            (793, "Puolukkaturvekangas (I)"),
            (794, "Puolukkaturvekangas (II)"),
            (795, "Ruohoturvekangas"),
            (796, "Varputurvekangas"),
        ] {
            assert_eq!(
                SoilType::from_opt_code(Some(code)).name(),
                name,
                "maalajikoodi {code}"
            );
        }
    }

    #[test]
    fn every_fertility_class_code_is_mapped() {
        for (code, name) in [
            (1, "Lehto tai vastaava suo"),
            (2, "Lehtomainen kangas tai vastaava suo"),
            (3, "Tuore kangas tai vastaava suo"),
            (4, "Kuivahko kangas tai vastaava suo"),
            (5, "Kuiva kangas tai vastaava suo"),
            (6, "Karukko kangas tai vastaava suo"),
            (7, "Kalliomaa ja hietikko"),
            (8, "Lakimaa ja tunturi"),
            (9, "Kuivahko kangas tai sitä karumpi, paperi-ilmoitukset."),
            (10, "Tuore kangas tai sitä rehevämpi, paperi-ilmoitukset."),
        ] {
            assert_eq!(
                FertilityClass::from_opt_code(Some(code)).name(),
                name,
                "käsittelykykyyskoodi {code}"
            );
        }
        assert_eq!(FertilityClass::from_opt_code(Some(11)).name(), "Muu");
        assert_eq!(FertilityClass::from_opt_code(None).name(), "Muu");
    }

    #[test]
    fn main_group_codes_map_to_their_names() {
        assert_eq!(MainGroup::from_opt_code(Some(1)).name(), "Metsämaa");
        assert_eq!(MainGroup::from_opt_code(Some(2)).name(), "Kitumaa");
        assert_eq!(MainGroup::from_opt_code(Some(3)).name(), "Joutomaa");
        assert_eq!(
            MainGroup::from_opt_code(Some(4)).name(),
            "Muu metsätalousmaa"
        );
        assert_eq!(MainGroup::from_opt_code(Some(5)).name(), "Muu");
        assert_eq!(MainGroup::from_opt_code(None).name(), "Muu");
    }

    #[test]
    fn fertility_class_codes_map_to_their_names() {
        assert_eq!(
            FertilityClass::from_opt_code(Some(1)).name(),
            "Lehto tai vastaava suo"
        );
        assert_eq!(
            FertilityClass::from_opt_code(Some(5)).name(),
            "Kuiva kangas tai vastaava suo"
        );
        assert_eq!(
            FertilityClass::from_opt_code(Some(7)).name(),
            "Kalliomaa ja hietikko"
        );
        assert_eq!(
            FertilityClass::from_opt_code(Some(10)).name(),
            "Tuore kangas tai sitä rehevämpi, paperi-ilmoitukset."
        );
        assert_eq!(FertilityClass::from_opt_code(Some(11)).name(), "Muu");
        assert_eq!(FertilityClass::from_opt_code(None).name(), "Muu");
    }

    #[test]
    fn every_enum_variant_has_a_name() {
        let soils = [
            SoilType::Keskikarkea,
            SoilType::KarkeaMoreeni,
            SoilType::KarkeaLajittunut,
            SoilType::HienojakoinenKangasmaa,
            SoilType::HienojakoinenMoreeni,
            SoilType::HienojakoinenLajittunut,
            SoilType::Silttipitoinen,
            SoilType::Savimaa,
            SoilType::KivinenKeskikarkeaKangas,
            SoilType::KivinenKarkeaMoreeni,
            SoilType::KivinenKarkeaLajittunut,
            SoilType::KivinenHieno,
            SoilType::Turvemaa,
            SoilType::SaraTurve,
            SoilType::RahkaTurve,
            SoilType::PuuvaltainenTurve,
            SoilType::EroosioherkkäSara,
            SoilType::EroosioherkkäRahka,
            SoilType::MaatumatonSara,
            SoilType::MaatumatonRahka,
            SoilType::MustikkaKangasI,
            SoilType::MustikkaKangasII,
            SoilType::PuolukkaKangasI,
            SoilType::PuolukkaKangasII,
            SoilType::RuohoTurveKangas,
            SoilType::VarpuTurveKangas,
            SoilType::Muu,
        ];
        for soil in soils {
            assert!(!soil.name().is_empty(), "missing name for {soil:?}");
        }

        let groups = [
            MainGroup::MetsaMaa,
            MainGroup::KituMaa,
            MainGroup::Joutomaa,
            MainGroup::MuuMetsaTalous,
            MainGroup::Muu,
        ];
        for group in groups {
            assert!(!group.name().is_empty(), "missing name for {group:?}");
        }

        let fertility = [
            FertilityClass::Kalliomaa,
            FertilityClass::KangasKarukko,
            FertilityClass::KangasKuiva,
            FertilityClass::KangasKuivahko,
            FertilityClass::KangasKuivahkoTaiKaru,
            FertilityClass::KangasLehtomainen,
            FertilityClass::KangasTuore,
            FertilityClass::KangasTuoreTaiReheva,
            FertilityClass::Lakimaa,
            FertilityClass::Lehto,
            FertilityClass::Muu,
        ];
        for class in fertility {
            assert!(!class.name().is_empty(), "missing name for {class:?}");
        }
    }

    #[test]
    fn value_limits_are_ordered_by_harvest_type() {
        // Päätehakkuun ikäraja on aina harvennusta korkeampi
        for (uudistus, harvennus) in [
            (&LIMITS.uudistus.kuusi, &LIMITS.harvennus.kuusi),
            (&LIMITS.uudistus.manty, &LIMITS.harvennus.manty),
            (&LIMITS.uudistus.koivu, &LIMITS.harvennus.koivu),
        ] {
            assert!(uudistus.age > harvennus.age);
            assert!(uudistus.diameter > harvennus.diameter);
        }
    }

    #[test]
    fn names_are_human_readable() {
        assert_eq!(Species::Manty.name(), "Mänty");
        assert_eq!(Species::Kuusi.name(), "Kuusi");
        assert_eq!(Species::Koivu.name(), "Koivu");
        assert_eq!(Species::Muu.name(), "Muu");
        assert_eq!(TimberType::Kuitu.name(), "kuitu");
        assert_eq!(TimberType::Tukki.name(), "tukki");
        assert_eq!(Cutting::Uudistus.name(), "uudistus");
        assert_eq!(Cutting::Ensiharvennus.name(), "ensiharvennus");
        assert_eq!(Cutting::Harvennus.name(), "harvennus");
    }
}
