//! Koko laskentaketjun (kiinteistö -> kuviot -> arvon laskenta -> Excel/KML)
//! kattava integraatiotesti.
//!
//! Testi käyttää vain julkista `metsaval`-kirjaston API:a, joten se varmistaa,
//! että yksikkötestien lisäksi kokonaiskulku pysyy ennallaan.

use geo::{LineString, MultiPolygon, Polygon};
use metsaval::consts::Cutting;
use metsaval::model::{Stand, StandSummary, TreeStratum};
use metsaval::{excel, geo_utils, maps, valuation};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Apu
// ---------------------------------------------------------------------------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    /// Luo yksilöidyn hakemiston, jotta rinnakkain ajettavat testit eivät
    /// kirjoita samoihin tiedostoihin.
    fn new(label: &str) -> Self {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "metsaval_pipeline_{}_{}_{}",
            label,
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-6,
        "expected {expected}, got {actual}"
    );
}

fn polygon_at(x: f64, y: f64, size: f64) -> Polygon<f64> {
    Polygon::new(
        LineString::from(vec![
            (x, y),
            (x + size, y),
            (x + size, y + size),
            (x, y + size),
            (x, y),
        ]),
        vec![],
    )
}

/// Kiinteistö ETRS-TM35FIN koordinaatistossa (100 x 100 m)
fn property() -> MultiPolygon<f64> {
    MultiPolygon::new(vec![polygon_at(385_000.0, 6_700_000.0, 100.0)])
}

fn stand_feature_at(x: f64, y: f64, size: f64, properties: serde_json::Value) -> geojson::Feature {
    let coordinates: Vec<Vec<[f64; 2]>> = vec![
        polygon_at(x, y, size)
            .exterior()
            .coords()
            .map(|coord| [coord.x, coord.y])
            .collect(),
    ];

    serde_json::from_value(json!({
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": coordinates},
        "properties": properties,
    }))
    .unwrap()
}

/// Kiinteistön rajalla oleva kuvio-ominaisuus
fn stand_feature(properties: serde_json::Value) -> geojson::Feature {
    stand_feature_at(385_000.0, 6_700_000.0, 100.0, properties)
}

fn stratum(standid: i64, treespecies: i64) -> TreeStratum {
    TreeStratum {
        standid,
        treespecies,
        age: None,
        basalarea: None,
        stemcount: None,
        meandiameter: None,
        meanheight: None,
        volume: None,
        sawlogpercent: None,
        sawlogvolume: None,
        pulpwoodvolume: None,
        volumegrowth: None,
        stembiomass: None,
        stumpbiomass: None,
        branchbiomass: None,
        leafbiomass: None,
    }
}

fn stand_summary(standid: i64, fertility: i64) -> StandSummary {
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
        fertility_class: Some(fertility),
        main_group: Some(1),
        soil_type: Some(24),
    }
}

fn read_entry(path: &Path, name: &str) -> String {
    let file = fs::File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut content = String::new();
    let mut entry = archive.by_name(name).unwrap();
    std::io::Read::read_to_string(&mut entry, &mut content).unwrap();
    content
}

// ---------------------------------------------------------------------------
// Testit
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_from_property_to_exports() {
    let dir = TempDir::new("full");

    // 1. Kiinteistön palstat leikataan yhdeksi geometriaksi
    let property_geometry = property();
    assert_close(geo_utils::area_m2(&property_geometry), 10_000.0);
    assert_eq!(
        geo_utils::bounds(&property_geometry),
        Some((385_000.0, 6_700_000.0, 385_100.0, 6_700_100.0))
    );

    // 2. Metsäkeskuksen kuviot leikataan kiinteistöön
    let features = vec![
        stand_feature(json!({"standid": 100, "standnumber": 1, "cuttingtype": 5,
                   "cuttingproposalyear": 2030, "area": 1.5})),
        stand_feature(json!({"standid": 101, "standnumber": 2, "cuttingtype": 3,
                   "cuttingproposalyear": 2031, "area": 0.5})),
    ];

    let stands: Vec<Stand> = features
        .iter()
        .filter_map(|feature| geo_utils::clip_stand(feature, &property_geometry))
        .collect();
    assert_eq!(stands.len(), 2);
    assert_close(stands[0].intersection_area_m2, 10_000.0);
    assert_close(stands[0].intersection_area_ha, 1.0);
    assert_eq!(stands[0].standid, Some(100));
    assert_eq!(stands[1].standid, Some(101));

    // 3. Puustotiedot GeoPackagesta
    let mut kuusi = stratum(100, 2);
    kuusi.age = Some(55.0);
    kuusi.basalarea = Some(25.0);
    kuusi.stemcount = Some(1200.0);
    kuusi.meandiameter = Some(22.0);
    kuusi.meanheight = Some(16.0);
    kuusi.volume = Some(150.0);
    kuusi.sawlogpercent = Some(40.0);
    kuusi.sawlogvolume = Some(60.0);
    kuusi.pulpwoodvolume = Some(90.0);
    kuusi.volumegrowth = Some(15.0);

    let mut manty = stratum(101, 1);
    manty.age = Some(60.0);
    manty.basalarea = Some(20.0);
    manty.stemcount = Some(900.0);
    manty.meandiameter = Some(24.0);
    manty.meanheight = Some(17.0);
    manty.volume = Some(120.0);
    manty.sawlogpercent = Some(33.33);
    manty.sawlogvolume = Some(40.0);
    manty.pulpwoodvolume = Some(80.0);
    manty.volumegrowth = Some(12.0);

    let strata = vec![kuusi, manty];
    let stand_summaries = vec![stand_summary(100, 1), stand_summary(101, 1)];

    // 4. Arvon laskenta
    let species_summary = valuation::build_species_summary(&strata, &stands).unwrap();
    assert_eq!(species_summary.len(), 2);
    assert_close(species_summary[0].volume, 150.0);
    assert_close(species_summary[1].volume, 120.0);
    assert!(species_summary[0].value_eur > 0.0);
    assert!(species_summary[1].value_eur > 0.0);

    let timber_values = valuation::build_timber_values(&species_summary);
    assert_eq!(timber_values.len(), 4);

    let stand_values = valuation::build_stand_values(&species_summary, &stand_summaries, &stands);
    assert_eq!(stand_values.len(), 2);

    let first = &stand_values[0];
    assert_eq!(first.standnumber, Some(1));
    assert!(matches!(first.cutting, Some(Cutting::Uudistus)));
    assert_eq!(first.area, Some(1.5));
    assert_close(first.volume, 150.0);
    assert_close(first.sawlogvolume, 60.0);
    assert_close(first.pulpwoodvolume, 90.0);
    // 1,5 ha * 0,75 * (90 kuitua + 60 tukkia) * 0,5 €/m³ + 1,5 ha * (720 + 1120) €/ha
    assert_close(first.costs_eur, 2844.375);
    // 1,5 * 60 * 0,75 * 85,01 + 1,5 * 90 * 0,75 * 29,22
    assert_close(first.income_eur, 8696.7);
    assert_close(first.net_eur, first.income_eur - first.costs_eur);
    assert!(matches!(
        first.fertility_class,
        metsaval::consts::FertilityClass::Lehto
    ));

    let second = &stand_values[1];
    assert_eq!(second.standnumber, Some(2));
    assert!(matches!(second.cutting, Some(Cutting::Harvennus)));
    assert_eq!(second.area, Some(0.5));
    assert_close(second.volume, 120.0);
    // 0,5 * 80 * 0,3 * 68,83 + 0,5 * 40 * 0,1 * 68,83
    assert_close(second.income_eur, 963.62);
    // 0,5 * (80 * 0,3 + 40 * 0,1) * 0,5 + 0,5 * 396
    assert_close(second.costs_eur, 205.0);

    let raw_rows = valuation::build_raw_rows(&strata, &stands, &stand_summaries).unwrap();
    assert_eq!(raw_rows.len(), 2);
    assert_close(raw_rows[0].sawlogpercent.unwrap(), 40.0);

    let fixed_costs = valuation::build_fixed_costs(&stand_values);
    assert_close(fixed_costs, 2.0 * (603.0 + 306.0));

    let totals = valuation::compute_totals(&stands, &stand_values);
    assert_eq!(totals.stand_count, 2);
    assert_close(totals.area_ha, 2.0);
    assert_close(totals.volume_m3, 150.0 * 1.5 + 120.0 * 0.5);
    assert_close(totals.income_eur, 8696.7 + 963.62);

    // 5. Konsoliyhteenveto
    let summary = valuation::create_dataset_summary(&stand_values, &fixed_costs, &totals);
    assert!(summary.contains("Yhteenveto"));
    assert!(summary.contains("2,00 ha"));
    assert!(summary.contains("285,00 m³"));
    assert!(summary.contains("110,00 m³"), "tukkimäärä");
    assert!(summary.contains("175,00 m³"), "kuitumäärä");
    assert!(summary.contains("LASKENNALLINEN ARVO"));
    // 6 610,825 netto - 1 818 kiinteät kulut -> 4 793 pyöristettynä
    assert!(summary.contains("4 793"));

    // 6. KML-tason luonti
    let kml = maps::generate_kml(&features, &property_geometry, &stand_values).unwrap();
    assert_eq!(kml.matches("<Placemark>").count(), 3, "rajat + 2 kuvioita");
    assert!(kml.contains("<name>1</name>"));
    assert!(kml.contains("<name>2</name>"));
    assert!(kml.contains("<Data name=\"Hakkuuehdotus\">"));

    let kml_path = dir.file("kohde.kml");
    maps::write_kml(&kml, &kml_path).unwrap();
    let written = fs::read_to_string(&kml_path).unwrap();
    assert_eq!(written, kml);

    // 7. Excel-yhteenveto
    let excel_path = dir.file("kohde.xlsx");
    excel::export(
        &excel_path,
        &totals,
        &stand_values,
        &species_summary,
        &timber_values,
        &raw_rows,
        &fixed_costs,
    )
    .unwrap();

    let workbook = read_entry(&excel_path, "xl/workbook.xml");
    for sheet in [
        "Yhteenveto",
        "Kuvioittain",
        "Puulajeittain",
        "Puutavaralajeittain",
        "Raakadata",
        "Hinnasto",
    ] {
        assert!(workbook.contains(&format!("name=\"{sheet}\"")), "{sheet}");
    }

    // Tunnusluvut löytyvät Yhteenveto-välilehdeltä
    let summary_sheet = read_entry(&excel_path, "xl/worksheets/sheet1.xml");
    assert!(summary_sheet.contains(">2</v>"), "kuvioiden määrä");
    assert!(summary_sheet.contains(">285</v>"), "puuston kokonaismäärä");
    let strings = read_entry(&excel_path, "xl/sharedStrings.xml");
    assert!(strings.contains("Kuvioiden määrä"));
    assert!(strings.contains("Kuvio ID"));
}

#[test]
fn property_without_stands_produces_no_stand_rows() {
    let property_geometry = property();
    let far_away = stand_feature_at(
        500_000.0,
        6_900_000.0,
        100.0,
        json!({"standid": 100, "area": 1.0}),
    );

    assert!(geo_utils::clip_stand(&far_away, &property_geometry).is_none());

    // Tyhjällä aineistolla kaikki tunnusluvut ovat nollia
    let totals = valuation::compute_totals(&[], &[]);
    assert_eq!(totals.stand_count, 0);
    assert_eq!(totals.area_ha, 0.0);
    assert_close(totals.net_eur, 0.0);
}

/// KML- ja Excel-tiedosto kirjoitetaan rinnakkain `join!`:lla. Molempien
/// tehtävien työ on synkronista, joten ne ajetaan `spawn_blocking`-
/// taustasäikeissä eivätkä ne blokkaa toisiaan.
#[tokio::test]
async fn kml_and_excel_are_written_concurrently() {
    let dir = TempDir::new("concurrent");
    let kml_path = dir.file("rinnakkain.kml");
    let excel_path = dir.file("rinnakkain.xlsx");

    let totals = valuation::compute_totals(&[], &[]);

    let (kml_result, excel_result) = tokio::join!(
        maps::generate_and_write_kml(vec![], property(), vec![], kml_path.clone()),
        excel::export_async(
            excel_path.clone(),
            totals,
            vec![],
            vec![],
            vec![],
            vec![],
            0.0,
        ),
    );

    kml_result.unwrap();
    excel_result.unwrap();

    let kml = fs::read_to_string(&kml_path).unwrap();
    assert!(kml.contains("<name>Kiinteistön rajat</name>"));

    let workbook = read_entry(&excel_path, "xl/workbook.xml");
    assert!(workbook.contains("name=\"Yhteenveto\""));
}
