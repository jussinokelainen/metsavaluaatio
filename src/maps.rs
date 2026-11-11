use crate::consts::{EPSG_TM35FIN, EPSG_WGS84, Species};
use crate::consts::{RED, RESET, YELLOW};
use crate::geo_utils;
use crate::model::StandValue;
use color_eyre::Result;
use geo::{MultiPolygon, Polygon};
use geojson::Feature;
use proj::Proj;
use rand::RngExt;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Luo metsäkuvion koordinaatistosta kml tiedostoon xml:laa
fn create_placemark(property: &Polygon<f64>, values: &StandValue) -> String {
    let name = if let Some(id) = values.standnumber {
        id
    } else {
        values.standid
    };
    // luo uusi placemarkin alku
    let mut kml = new_placemark_start(name.to_string());

    // lisää data tiedostoon
    kml.push_str("            <ExtendedData>\n");

    kml.push_str(&new_data(
        "Tulot",
        Some(format!("{:.2} €", values.income_eur)),
    ));
    kml.push_str(&new_data(
        "Menot",
        Some(format!("{:.2} €", values.costs_eur)),
    ));
    kml.push_str(&new_data("Netto", Some(format!("{:.2} €", values.net_eur))));
    kml.push_str(&new_data("Pinta-ala (ha)", values.area));
    kml.push_str(&new_data("Runkoluku kpl/ha", values.stemcount));
    kml.push_str(&new_data("Keski-ikä", values.meanage));
    kml.push_str(&new_data("Keskiläpimitta", values.meandiameter));
    kml.push_str(&new_data("Keskipituus", values.meanheight));
    kml.push_str(&new_data(
        "Tilavuus m³/ha",
        Some(format!("{:.2}", values.volume)),
    ));
    kml.push_str(&new_data(
        "Tilavuuskasvuprosentti",
        values.growth_pct.map(|p| format!("{:.2}", p)),
    ));

    kml.push_str(&new_data(
        "Tukkiprosentti",
        Some(format!(
            "{:.2}",
            (values.sawlogvolume / (values.sawlogvolume + values.pulpwoodvolume)) * 100.0
        )),
    ));
    if let Some(code) = values.main_species {
        kml.push_str(&new_data(
            "Pääpuulaji",
            Some(Species::from_code(code).name()),
        ));
    }
    if let Some(cutting) = values.cutting {
        kml.push_str(&new_data("Hakkuuehdotus", Some(cutting.name())));
        kml.push_str(&new_data("Ehdotusvuosi", values.cuttingproposalyear));
    }

    kml.push_str(&new_data(
        "Kasvupaikka",
        Some(values.fertility_class.name()),
    ));
    kml.push_str(&new_data("Maaluokka", Some(values.main_group.name())));
    kml.push_str(&new_data("Maalaji", Some(values.soil_type.name())));

    kml.push_str("            </ExtendedData>\n");

    // lisää koordinaatit tiedostoon
    kml.push_str("            <Polygon><outerBoundaryIs><LinearRing><coordinates>\n");
    for coord in property.exterior().coords() {
        let _ = writeln!(kml, "                {},{},0", coord.x, coord.y);
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
            let _ = writeln!(kml, "                    {},{},0", coord.x, coord.y);
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
///
/// # Examples
/// ```
/// # use::metsaval::maps::xml_escape;
/// let s = "& < > \" '";
/// assert_eq!(xml_escape(s), "&amp; &lt; &gt; &quot; &apos;")
/// ```
pub fn xml_escape(value: &str) -> String {
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
        "75b8f6", "77dd77", "87e0cf", "89cff0", "90ee90", "98ff98", "9adedb", "a2bffe", "add8e6",
        "b39eb5", "bdb0d0", "cef0cc", "d8a1c4", "d9bbc8", "dcd0ff", "dea5a4", "f08080", "f3e5ab",
        "f49ac2", "f4c2c2", "f7cac1", "ff6961", "ff91a4", "ff964f", "ffb6c1", "ffbcd9", "ffddf4",
        "ffe4e1", "fffaa0", "fffacd",
    ];
    let mut rng = rand::rng();
    let idx = rng.random_range(0..colors.len());

    colors[idx]
}

/// Luo kml tiedoston jonka voi importata google mapsiin tasoksi
pub fn generate_kml(
    features: &[Feature],
    property: &MultiPolygon<f64>,
    stand_values: &[StandValue],
) -> Result<String> {
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

    // Hakemisto välttää lineaarisen haun jokaista kuviota kohden.
    let values_by_stand: HashMap<i64, &StandValue> = stand_values
        .iter()
        .map(|values| (values.standid, values))
        .collect();

    for feature in features {
        // Yksi leikkaus riittää: saadaan sekä kuvion tiedot että geometria.
        let Some((stand, geometry)) = geo_utils::clip_stand_with_geometry(feature, property) else {
            continue;
        };

        let Some(stand_id) = stand.standid else {
            continue;
        };

        let Some(values) = values_by_stand.get(&stand_id) else {
            println!(
                "{RED}Tietoa ei löytynyt metsäkuviolle {stand_id}{RESET}, tuntematon puulaji?"
            );
            continue;
        };

        let mp_wgs84 = geo_utils::transform(&geometry, &to_wgs84);

        for polygon in &mp_wgs84.0 {
            kml.push_str(&create_placemark(polygon, values));
        }
    }

    kml.push_str(
        "    </Document>
</kml>",
    );

    Ok(kml)
}

pub fn write_kml(kml_data: &str, file_path: &Path) -> Result<()> {
    std::fs::write(file_path, kml_data)?;

    Ok(())
}

/// Generoi KML-dokumentin ja kirjoittaa sen levylle.
///
/// Kokoaminen ja tiedoston kirjoittaminen ovat synkronisia ja hieman
/// CPU:ta kuormittavia, joten ne ajetaan `spawn_blocking`-taustasäikeessä.
/// Tämä pitää asynkronisen runtimen vapaana ja mahdollistaa samanaikaisen
/// Excel-kirjoituksen. Palauttaa tiedoston polun, jotta kutsuja ei tarvitse
/// pitää siitä omaa kopioitaan.
///
/// Huomio: taustasäikeen suoritusta ei voi peruuttaa - se jatkaa
/// tiedostoon kirjoittamista, vaikka kutsuja lopettaisi odottamisen.
pub async fn generate_and_write_kml(
    features: Vec<Feature>,
    property: MultiPolygon<f64>,
    stand_values: Vec<StandValue>,
    file_path: PathBuf,
) -> Result<PathBuf> {
    // `move` siirtää omistajuuden sulkeumaan, koska taustasäikeen on
    // oltava 'static - se ei saa viitata kutsujan pinoon.
    tokio::task::spawn_blocking(move || -> Result<PathBuf> {
        let kml_data = generate_kml(&features, &property, &stand_values)?;
        write_kml(&kml_data, &file_path)?;
        Ok(file_path)
    })
    // `.await` on `Result<Result<PathBuf, Report>, JoinError>`: yksi `?`
    // poistaa taustasäikeen `JoinError`in (säikeen kaatuminen tai peruutus)
    // ja muuntaa sen virheeksi. Jäljelle jäävä `Result` on suoraan
    // funktion paluuarvo.
    .await?
}

fn hyperlink(url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{url}\x1b]8;;\x1b\\")
}

pub fn print_maps_url(url: &str) {
    println!("\nLinkki maps tason luontiin:");
    println!("{YELLOW}{}{RESET}\n", hyperlink(url));
}

/// Muodostaa komennon joka avaa annetun url:in selaimessa. Palauttaa komennon
/// suorittamatta sitä, jotta komennon voi testata erikseen.
fn browser_command(url: &str) -> std::process::Command {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    command.arg(url);
    command
}

/// Avaa selaimen annetulla url:lla, tai printtaa linkin jos selaimen
/// avaaminen ei onnistu
pub fn open_browser(url: &str) {
    open_browser_with(url, browser_command(url));
}

/// Suorittaa annetun komennon. Jos komennon suorittaminen epäonnistuu,
/// tulostaa linkin käyttäjän avattavaksi. Erittely `open_browser`:
///sta mahdollistaa molemmat haarat testattaessa ilman oikeaa selainta.
fn open_browser_with(url: &str, mut command: std::process::Command) {
    match command.spawn() {
        Ok(_) => {}
        Err(_) => {
            println!("Selaimen avaaminen epäonnistui!");
            print_maps_url(url);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{Cutting, FertilityClass, MainGroup, SoilType};
    use geo::{LineString, Polygon};
    use serde_json::json;

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

    fn feature_with_geometry(standid: Option<i64>, polygon: Polygon<f64>) -> Feature {
        let coordinates: Vec<Vec<[f64; 2]>> = vec![
            polygon
                .exterior()
                .coords()
                .map(|coord| [coord.x, coord.y])
                .collect(),
        ];

        let geometry = json!({"type": "Polygon", "coordinates": coordinates});
        let properties = match standid {
            Some(standid) => json!({"standid": standid, "standnumber": 7}),
            None => json!({"standnumber": 7}),
        };

        serde_json::from_value(json!({
            "type": "Feature",
            "geometry": geometry,
            "properties": properties,
        }))
        .unwrap()
    }

    fn stand_feature(standid: i64) -> Feature {
        feature_with_geometry(Some(standid), polygon_at(385_000.0, 6_700_000.0, 100.0))
    }

    fn stand_value(standid: i64) -> StandValue {
        StandValue {
            standid,
            standnumber: Some(7),
            cutting: Some(Cutting::Uudistus),
            cuttingproposalyear: Some(2030),
            volume: 150.0,
            sawlogvolume: 60.0,
            pulpwoodvolume: 90.0,
            volumegrowth: 15.0,
            meanage: Some(55.0),
            stemcount: Some(1200.0),
            meandiameter: Some(22.0),
            meanheight: Some(16.0),
            area: Some(1.5),
            growth_pct: Some(10.0),
            main_species: Some(2),
            fertility_class: FertilityClass::Lehto,
            main_group: MainGroup::MetsaMaa,
            soil_type: SoilType::Savimaa,
            income_eur: 1000.0,
            costs_eur: 200.0,
            net_eur: 800.0,
        }
    }

    fn placemark_count(kml: &str) -> usize {
        kml.matches("<Placemark>").count()
    }

    #[test]
    fn generate_kml_builds_an_importable_document() {
        let kml = generate_kml(&[stand_feature(11)], &property(), &vec![stand_value(11)]).unwrap();

        assert!(kml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(kml.contains("<kml xmlns=\"http://www.opengis.net/kml/2.2\">"));
        assert!(kml.ends_with("</kml>"));
        assert!(kml.contains("<Document>"));
        assert!(kml.contains("</Document>"));

        // Kiinteistön rajat + yksi kuvio
        assert_eq!(placemark_count(&kml), 2);
        assert!(kml.contains("<name>Kiinteistön rajat</name>"));
        assert!(kml.contains("<name>7</name>"));
    }

    #[test]
    fn generate_kml_writes_stand_attributes_as_extended_data() {
        let kml = generate_kml(&[stand_feature(11)], &property(), &vec![stand_value(11)]).unwrap();

        for (field, value) in [
            ("Tulot", "1000.00 €"),
            ("Menot", "200.00 €"),
            ("Netto", "800.00 €"),
            ("Pinta-ala (ha)", "1.5"),
            ("Runkoluku kpl/ha", "1200"),
            ("Keski-ikä", "55"),
            ("Keskiläpimitta", "22"),
            ("Keskipituus", "16"),
            ("Tilavuus m³/ha", "150.00"),
            ("Tilavuuskasvuprosentti", "10.00"),
            ("Tukkiprosentti", "40.00"),
            ("Pääpuulaji", "Kuusi"),
            ("Hakkuuehdotus", "uudistus"),
            ("Ehdotusvuosi", "2030"),
            ("Kasvupaikka", "Lehto tai vastaava suo"),
            ("Maaluokka", "Metsämaa"),
            ("Maalaji", "Savimaa"),
        ] {
            assert!(
                kml.contains(&format!("<Data name=\"{field}\">")),
                "kenttä {field} puuttuu"
            );
            assert!(
                kml.contains(&format!("<value>{value}</value>")),
                "arvo {value} kentässä {field} puuttuu"
            );
        }
    }

    #[test]
    fn generate_kml_omits_empty_values() {
        let mut value = stand_value(11);
        value.area = None;
        value.meanage = None;
        value.cutting = None;
        value.cuttingproposalyear = None;
        value.growth_pct = None;
        value.main_species = None;

        let kml = generate_kml(&[stand_feature(11)], &property(), &vec![value]).unwrap();
        for field in [
            "Pinta-ala (ha)",
            "Keski-ikä",
            "Hakkuuehdotus",
            "Ehdotusvuosi",
            "Tilavuuskasvuprosentti",
            "Pääpuulaji",
        ] {
            assert!(
                !kml.contains(&format!("<Data name=\"{field}\">")),
                "kenttä {field} pitäisi jättää pois"
            );
        }
    }

    #[test]
    fn generate_kml_falls_back_to_standid_without_standnumber() {
        let mut value = stand_value(42);
        value.standnumber = None;

        let kml = generate_kml(&[stand_feature(42)], &property(), &vec![value]).unwrap();
        assert!(kml.contains("<name>42</name>"));
        assert!(!kml.contains("<name>7</name>"));
    }

    #[test]
    fn generate_kml_skips_stands_without_matching_values() {
        let kml = generate_kml(
            &[stand_feature(11), stand_feature(99)],
            &property(),
            &vec![stand_value(11)],
        )
        .unwrap();
        assert_eq!(placemark_count(&kml), 2, "tuntematon kuvio ohitetaan");
    }

    #[test]
    fn generate_kml_skips_stands_outside_the_property() {
        let far_away = feature_with_geometry(Some(11), polygon_at(500_000.0, 6_900_000.0, 100.0));
        let kml = generate_kml(&[far_away], &property(), &vec![stand_value(11)]).unwrap();
        assert_eq!(placemark_count(&kml), 1, "vain kiinteistön rajat");
    }

    #[test]
    fn generate_kml_skips_features_without_geometry_or_standid() {
        let mut without_geometry = stand_feature(11);
        without_geometry.geometry = None;

        let no_id = feature_with_geometry(None, polygon_at(385_000.0, 6_700_000.0, 100.0));

        let kml = generate_kml(
            &[without_geometry, no_id],
            &property(),
            &vec![stand_value(11)],
        )
        .unwrap();
        assert_eq!(placemark_count(&kml), 1, "vain kiinteistön rajat");
    }

    #[test]
    fn generate_kml_projects_coordinates_to_wgs84() {
        let kml = generate_kml(&[stand_feature(11)], &property(), &vec![stand_value(11)]).unwrap();

        assert!(
            !kml.contains("385000"),
            "TM35FIN-koordinaatteja ei saa jäädä"
        );
        let has_degree_coordinates = kml.lines().any(|line| {
            let parts: Vec<&str> = line.trim().split(',').collect();
            parts.len() == 3
                && parts[0]
                    .parse::<f64>()
                    .is_ok_and(|lon| (15.0..35.0).contains(&lon))
                && parts[1]
                    .parse::<f64>()
                    .is_ok_and(|lat| (59.0..71.0).contains(&lat))
                && parts[2] == "0"
        });
        assert!(has_degree_coordinates, "koordinaatteja ei löytynyt");
    }

    #[test]
    fn write_kml_creates_the_file() {
        let dir = crate::test_support::TempDir::new("maps");
        let path = dir.file("test.kml");
        write_kml("<kml/>", &path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<kml/>");
    }

    #[tokio::test]
    async fn generate_and_write_kml_writes_the_file() {
        let dir = crate::test_support::TempDir::new("maps_async");
        let path = dir.file("async.kml");

        let written = generate_and_write_kml(
            vec![stand_feature(11)],
            property(),
            vec![stand_value(11)],
            path.clone(),
        )
        .await
        .unwrap();

        assert_eq!(written, path, "palautettava polku on annettu polku");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<kml xmlns=\"http://www.opengis.net/kml/2.2\">"));
        assert_eq!(placemark_count(&content), 2, "rajat + kuvio");
    }

    #[test]
    fn generate_kml_without_stands_only_outlines_the_property() {
        let kml = generate_kml(&[], &property(), &vec![]).unwrap();
        assert_eq!(placemark_count(&kml), 1);
        assert!(kml.contains("<name>Kiinteistön rajat</name>"));
        assert!(kml.contains("<Document>"));
        assert!(kml.contains("</kml>"));
    }

    #[test]
    fn new_data_formats_the_xml_field() {
        let field = new_data("Pinta-ala (ha)", Some(1.5));
        assert!(field.contains("<Data name=\"Pinta-ala (ha)\">"));
        assert!(field.contains("<value>1.5</value>"));

        assert_eq!(new_data::<f64>("Puuttuva", None), "");
    }

    #[test]
    fn xml_escape_escapes_markup_characters() {
        assert_eq!(xml_escape("plain text"), "plain text");
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("100% <ok>"), "100% &lt;ok&gt;");
        assert_eq!(xml_escape(r#"say "hi""#), "say &quot;hi&quot;");
        assert_eq!(xml_escape("it's"), "it&apos;s");
        // Ensin & merkki, muuten escapet muuttuvat toistamisen yhteydessä
        assert_eq!(xml_escape("&amp;"), "&amp;amp;");
        assert_eq!(xml_escape("<>&\"'"), "&lt;&gt;&amp;&quot;&apos;");
    }

    #[test]
    fn property_outline_includes_every_polygon() {
        let multi = MultiPolygon::new(vec![
            polygon_at(385_000.0, 6_700_000.0, 100.0),
            polygon_at(385_500.0, 6_700_500.0, 50.0),
        ]);
        let outline = add_property_outline(&multi);
        assert_eq!(outline.matches("<Polygon>").count(), 2);
        assert!(outline.contains("Kiinteistön rajat"));
        assert!(outline.contains("</MultiGeometry>"));
    }

    #[test]
    fn clip_stand_with_geometry_clips_to_the_property() {
        let overlapping = stand_feature(11);
        let (stand, clipped) =
            geo_utils::clip_stand_with_geometry(&overlapping, &property()).unwrap();
        assert_eq!(stand.standid, Some(11));
        assert!((geo_utils::area_m2(&clipped) - 10_000.0).abs() < 1e-6);

        let outside = feature_with_geometry(Some(11), polygon_at(500_000.0, 6_900_000.0, 100.0));
        assert!(geo_utils::clip_stand_with_geometry(&outside, &property()).is_none());

        let mut point = stand_feature(11);
        point.geometry = Some(geojson::Geometry::new(
            serde_json::from_value(json!({"type": "Point", "coordinates": [1.0, 2.0]})).unwrap(),
        ));
        assert!(geo_utils::clip_stand_with_geometry(&point, &property()).is_none());
    }

    /// Luo Featuren, jonka geometria on MultiPolygon.
    fn multipolygon_feature(standid: i64) -> Feature {
        let square = polygon_at(385_000.0, 6_700_000.0, 50.0);
        let coordinates: Vec<Vec<Vec<[f64; 2]>>> = vec![vec![
            square
                .exterior()
                .coords()
                .map(|coord| [coord.x, coord.y])
                .collect(),
        ]];

        serde_json::from_value(json!({
            "type": "Feature",
            "geometry": {"type": "MultiPolygon", "coordinates": coordinates},
            "properties": {"standid": standid, "standnumber": 7},
        }))
        .unwrap()
    }

    #[test]
    fn clip_stand_with_geometry_accepts_multipolygon_geometries() {
        let (_, clipped) =
            geo_utils::clip_stand_with_geometry(&multipolygon_feature(11), &property()).unwrap();
        assert!((geo_utils::area_m2(&clipped) - 2_500.0).abs() < 1e-6);

        let kml = generate_kml(
            &[multipolygon_feature(11)],
            &property(),
            &vec![stand_value(11)],
        )
        .unwrap();
        assert_eq!(placemark_count(&kml), 2, "rajat + kuvio");
    }

    #[test]
    fn generate_kml_skips_stands_below_the_minimum_intersection_area() {
        // Leikkaus vain 0.005 m × 100 m = 0.5 m² (< 1 m²), joten clip_stand
        // hylkää kuvion vaikka geometria leikkaakin
        let sliver = feature_with_geometry(Some(11), polygon_at(385_099.995, 6_700_000.0, 100.0));

        assert!(
            geo_utils::clip_stand_with_geometry(&sliver, &property()).is_none(),
            "liian pieni leikkaus hylätään"
        );

        let kml = generate_kml(&[sliver], &property(), &vec![stand_value(11)]).unwrap();
        assert_eq!(placemark_count(&kml), 1, "vain kiinteistön rajat");
    }

    #[test]
    fn hyperlink_wraps_the_url_in_an_osc8_sequence() {
        assert_eq!(
            hyperlink("https://example.com/mymaps"),
            "\x1b]8;;https://example.com/mymaps\x1b\\https://example.com/mymaps\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn print_maps_url_prints_the_hyperlink() {
        print_maps_url("https://www.google.com/mymaps");
    }

    #[test]
    fn browser_command_opens_the_url_with_the_platform_program() {
        let command = browser_command("https://www.google.com/mymaps");
        let debug = format!("{command:?}");

        let expected_program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        assert!(
            debug.contains(expected_program),
            "komento: {debug} (odotettu {expected_program})"
        );
        assert!(
            debug.contains("https://www.google.com/mymaps"),
            "komento: {debug}"
        );
    }

    #[test]
    fn open_browser_with_prints_the_link_when_the_command_fails() {
        // Olematon ohjelma aiheuttaa spawn-virheen, jolloin käyttäjä
        // saa linkin avattavaksi itse
        open_browser_with(
            "https://www.google.com/mymaps",
            std::process::Command::new("metsaval_ei_olemassa_oleva_ohjelma"),
        );
    }

    #[test]
    fn open_browser_with_succeeds_with_a_working_command() {
        // "true" on aina olemassa unix-järjestelmillä ja se ei tee mitään
        open_browser_with("https://example.com", std::process::Command::new("true"));
    }
}
