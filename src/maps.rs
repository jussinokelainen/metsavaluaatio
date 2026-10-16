use crate::consts::{CYAN, Cutting, RESET, YELLOW};
use crate::consts::{EPSG_TM35FIN, EPSG_WGS84};
use crate::model::{Stand, StandSummary};
use crate::{geo_utils, metsakeskus};
use color_eyre::Result;
use geo::{BooleanOps, MultiPolygon, Polygon};
use geojson::Feature;
use proj::Proj;
use rand::RngExt;
use std::path::PathBuf;

/// Luo metsäkuvion koordinaatistosta kml tiedostoon xml:laa
fn create_placemark(property: &Polygon<f64>, summary: &StandSummary, stand: &Stand) -> String {
    // luo uusi placemarkin alku
    let mut kml = new_placemark_start(summary.standid.to_string());

    // lisää data tiedostoon
    kml.push_str("            <ExtendedData>\n");

    kml.push_str(&new_data("Pinta-ala (ha)", stand.area));
    kml.push_str(&new_data("Runkoluku kpl/ha", summary.stemcount));
    kml.push_str(&new_data("Keski-ikä", summary.meanage));
    kml.push_str(&new_data("Keskiläpimitta", summary.meandiameter));
    kml.push_str(&new_data("Keskipituus", summary.meanheight));
    kml.push_str(&new_data("Tilavuuskasvuprosentti", summary.volumegrowth));
    if let Some(saw_vol) = summary.sawlogvolume
        && let Some(pulp_vol) = summary.pulpwoodvolume
    {
        // Kerrotaan arvo 10000:lla josta floor() ja jaetaan 100:lla jotta
        // prosenttiarvo on 2 desimaalin tarkkuudella
        let saw_perc = ((saw_vol / (saw_vol + pulp_vol)) * 10000.0).floor() / 100.0;
        kml.push_str(&new_data("Tukkiprosentti", Some(saw_perc)));
    }
    if let Some(cutting) = stand.cutting() {
        kml.push_str(&new_data("Hakkuuehdotus", Some(Cutting::name(cutting))));
        kml.push_str(&new_data("Ehdotusvuosi", stand.cuttingproposalyear));
    }

    kml.push_str("            </ExtendedData>\n");

    // lisää koordinaatit tiedostoon
    kml.push_str("            <Polygon><outerBoundaryIs><LinearRing><coordinates>\n");
    for coord in property.exterior().coords() {
        kml.push_str(&format!("                {},{},0\n", coord.x, coord.y));
    }
    kml.push_str(
        "            </coordinates></LinearRing></outerBoundaryIs></Polygon>
        </Placemark>\n",
    );

    kml
}

fn add_property_outline(property: &MultiPolygon) -> String {
    // luo uusi placemarkin alku
    let mut kml = String::from(
        "        <Placemark>
            <name>Kiinteistön rajat</name>
            <Style>
                <LineStyle>
                    <color>ff000000</color>
                    <width>2</width>
                </LineStyle>
                <PolyStyle>
                    <fill>0</fill>
                </PolyStyle>
            </Style>
            <MultiGeometry>\n",
    );

    // lisää koordinaatit tiedostoon
    for polygon in &property.0 {
        kml.push_str("                <Polygon><outerBoundaryIs><LinearRing><coordinates>\n");
        for coord in polygon.exterior().coords() {
            kml.push_str(&format!("                    {},{},0\n", coord.x, coord.y));
        }
        kml.push_str("                </coordinates></LinearRing></outerBoundaryIs></Polygon>\n");
    }
    kml.push_str(
        "            </MultiGeometry>
        </Placemark>\n",
    );

    kml
}

/// Luo placemarkkiin lisättävän <Data> kentän annetulla nimellä ja arvolla
fn new_data<T: std::fmt::Display>(name: &str, value: Option<T>) -> String {
    if let Some(data) = value {
        format!(
            "                <Data name=\"{name}\">
                    <value>{data}</value>
                </Data>\n"
        )
    } else {
        String::new()
    }
}

/// Luo uuden placemarkin alun satunnaisella värillä ja annetulla nimellä
fn new_placemark_start(name: String) -> String {
    format!(
        "        <Placemark>
            <name>{}</name>
            <Style>
                <PolyStyle>
                    <color>7f{}</color>
                    <fill>1</fill>
                    <outline>1</outline>
                </PolyStyle>
            </Style>\n",
        xml_escape(&name),
        random_color()
    )
}

/// Muuttaa merkkijonon xml-kelpoiseksi
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Antaa satunnaisen hex värikoodin
fn random_color() -> &'static str {
    let colors = [
        "75b8f6", "77dd77", "87e0cf", "89cff0", "90ee90", "98ff98", "99c5c4", "9adedb", "a2bffe",
        "add8e6", "b2fba5", "b39eb5", "bdb0d0", "cb99c9", "cef0cc", "d8a1c4", "d9bbc8", "dcd0ff",
        "dea5a4", "f08080", "f3e5ab", "f49ac2", "f4c2c2", "f7cac1", "ff6961", "ff91a4", "ff964f",
        "ffb6c1", "ffbcd9", "ffddf4", "ffe4e1", "fffaa0", "fffacd",
    ];
    let mut rng = rand::rng();
    let idx = rng.random_range(0..colors.len());

    colors[idx]
}

/// Palauttaa metsäkuvion jos se on annetulla palstalla, muuten palauttaa None
fn clip_stand_geometry(
    feature: &geojson::Feature,
    property: &geo::MultiPolygon<f64>,
) -> Option<geo::MultiPolygon<f64>> {
    let geometry = feature.geometry.as_ref()?;
    let stand_geometry: geo::Geometry<f64> = geometry.value.clone().try_into().ok()?;

    let stand_polygon = match stand_geometry {
        geo::Geometry::Polygon(polygon) => geo::MultiPolygon(vec![polygon]),
        geo::Geometry::MultiPolygon(multipolygon) => multipolygon,
        _ => return None,
    };

    let intersection = stand_polygon.intersection(property);

    if intersection.0.is_empty() {
        return None;
    }

    Some(intersection)
}

/// Luo kml tiedoston jonka voi importata google mapsiin tasoksi
pub fn write_kml(
    features: Vec<Feature>,
    property: &MultiPolygon<f64>,
    file_path: &str,
    gpkg_path: PathBuf,
) -> Result<()> {
    let mut kml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
    <Document>
"#,
    );

    let to_wgs84 = Proj::new_known_crs(EPSG_TM35FIN, EPSG_WGS84, None)?;
    kml.push_str(&add_property_outline(&geo_utils::transform(
        property, &to_wgs84,
    )));
    for feature in &features {
        let Some(geometry) = clip_stand_geometry(feature, property) else {
            continue;
        };

        let Some(stand) = geo_utils::clip_stand(feature, property) else {
            continue;
        };

        let Some(stand_id) = stand.standid else {
            continue;
        };

        let (_, stand_summaries) = metsakeskus::read_trees(&gpkg_path, &[stand_id])?;

        let mp_wgs84 = geo_utils::transform(&geometry, &to_wgs84);

        for polygon in &mp_wgs84.0 {
            kml.push_str(&create_placemark(polygon, &stand_summaries[0], &stand));
        }
    }

    kml.push_str(
        "    </Document>
</kml>",
    );

    std::fs::write(&file_path, kml)?;
    match std::fs::canonicalize(file_path) {
        Ok(path) => {
            println!(
                "Google maps taso tallennettu: {CYAN}{}{RESET}",
                path.display()
            );
        }
        Err(_) => println!("Google maps taso tallennettu: {CYAN}{file_path}{RESET}"),
    }
    Ok(())
}

/// Avaa selaimen annetulla url:lla, tai printtaa linkin jos selaimen
/// avaaminen ei onnistu
pub fn open_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    match command.arg(url).spawn() {
        Ok(_) => {}
        Err(_) => {
            println!("Selaimen avaaminen epäonnistui, maps sivun linkki:");
            println!("{YELLOW}{url}{RESET}\n");
        }
    }
}
