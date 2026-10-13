use crate::{EPSG_TM35FIN, EPSG_WGS84, geo_utils};
use color_eyre::Result;
use geo::{BooleanOps, MultiPolygon, Polygon};
use geojson::Feature;
use proj::Proj;

/// Luo metsäkuvion koordinaatistosta kml tiedostoon xml:laa
fn create_placemark(feature: &Feature, property: &Polygon<f64>) -> String {
    let name = feature
        .properties
        .as_ref()
        .and_then(|properties| properties.get("K_TUNNUS"))
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Unknown stand".to_string());

    let mut kml = format!(
        "<Placemark><name>{}</name><Polygon>\
         <outerBoundaryIs><LinearRing><coordinates>",
        xml_escape(&name)
    );

    for coord in property.exterior().coords() {
        kml.push_str(&format!("{},{},0 ", coord.x, coord.y));
    }

    kml.push_str("</coordinates></LinearRing></outerBoundaryIs></Polygon></Placemark>");

    kml
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
) -> Result<()> {
    let mut kml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
<Document>"#,
    );

    for feature in &features {
        let Some(geometry) = clip_stand_geometry(feature, property) else {
            continue;
        };

        let to_wgs84 = Proj::new_known_crs(EPSG_TM35FIN, EPSG_WGS84, None)?;
        let mp_wgs84 = geo_utils::transform(&geometry, &to_wgs84);

        for polygon in &mp_wgs84.0 {
            kml.push_str(&create_placemark(feature, polygon));
        }
    }

    kml.push_str("</Document></kml>");

    std::fs::write(&file_path, kml)?;
    println!("Google maps taso: {file_path}");
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
            println!("{url}\n");
        }
    }
}
