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
}

impl Species {
    /// Metsäkeskuksen `treespecies`-koodi -> puulaji.
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::Manty),
            2 => Some(Self::Kuusi),
            29 => Some(Self::Koivu),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Manty => "Mänty",
            Self::Kuusi => "Kuusi",
            Self::Koivu => "Koivu",
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
