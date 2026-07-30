use crate::consts::MIN_INTERSECTION_AREA_M2;
use crate::model::Stand;
use geo::{BooleanOps, HasDimensions, MultiPolygon};
use proj::Proj;
use serde_json::Value;

/// Muuntaa GeoJSON-piirteen geometrian MultiPolygoniksi.
pub fn feature_to_multipolygon(feature: &geojson::Feature) -> Option<MultiPolygon<f64>> {
    let gj_geometry = feature.geometry.as_ref()?;
    let geometry: geo::Geometry<f64> = geo::Geometry::try_from(gj_geometry.clone()).ok()?;

    match geometry {
        geo::Geometry::Polygon(polygon) => Some(MultiPolygon::new(vec![polygon])),
        geo::Geometry::MultiPolygon(multi) => Some(multi),
        _ => None,
    }
}

/// Yhdistää usean MultiPolygonin yhdeksi (shapely `unary_union`).
pub fn union_all(polygons: &[MultiPolygon<f64>]) -> MultiPolygon<f64> {
    geo::unary_union(polygons)
}

/// Muuntaa koordinaatit WGS84 -> ETRS-TM35FIN (tai päinvastoin).
pub fn transform(polygon: &MultiPolygon<f64>, proj: &Proj) -> MultiPolygon<f64> {
    use geo::MapCoords;
    polygon.map_coords(|coord| {
        let (x, y) = proj
            .convert((coord.x, coord.y))
            .expect("koordinaattimuunnos epäonnistui");
        (x, y).into()
    })
}

/// Laskee geometrian pinta-alan neliömetreinä (oletetaan metrinen CRS).
pub fn area_m2(polygon: &MultiPolygon<f64>) -> f64 {
    use geo::Area;
    polygon.unsigned_area()
}

/// Palauttaa geometrian rajat (minx, miny, maxx, maxy).
pub fn bounds(polygon: &MultiPolygon<f64>) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut seen = false;

    for poly in &polygon.0 {
        for coord in &poly.exterior().0 {
            seen = true;
            min_x = min_x.min(coord.x);
            min_y = min_y.min(coord.y);
            max_x = max_x.max(coord.x);
            max_y = max_y.max(coord.y);
        }
    }

    if seen {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Leikkaa yhden WFS-kuvion kiinteistön geometriaan ja palauttaa kuvion
/// Metsäkeskuksen ominaisuustiedoilla, jos leikkauspinta-ala on riittävä.
pub fn clip_stand(feature: &geojson::Feature, property: &MultiPolygon<f64>) -> Option<Stand> {
    let geometry = feature_to_multipolygon(feature)?;
    let intersection: MultiPolygon<f64> = geometry.intersection(property);
    let area = area_m2(&intersection);

    if intersection.is_empty() || area < MIN_INTERSECTION_AREA_M2 {
        return None;
    }

    let props = feature.properties.as_ref();

    Some(Stand {
        standid: props.and_then(|p| p.get("standid")).and_then(value_i64),
        standnumber: props.and_then(|p| p.get("standnumber")).and_then(value_i64),
        cuttingtype: props.and_then(|p| p.get("cuttingtype")).and_then(value_i64),
        cuttingproposalyear: props
            .and_then(|p| p.get("cuttingproposalyear"))
            .and_then(value_i64),
        area: props.and_then(|p| p.get("area")).and_then(value_f64),
        intersection_area_m2: area,
        intersection_area_ha: area / 10_000.0,
    })
}

/// JSON-arvo -> f64 (hyväksyy myös numeerisen merkkijonon).
pub fn value_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// JSON-arvo -> i64 (hyväksyy myös numeerisen merkkijonon).
pub fn value_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|f| f as i64)),
        Value::String(text) => text
            .trim()
            .parse()
            .ok()
            .or_else(|| text.trim().parse::<f64>().ok().map(|f| f as i64)),
        _ => None,
    }
}

/// Geometrian "allekirjoitus" tulostusta varten.
pub fn geometry_type(polygon: &MultiPolygon<f64>) -> &'static str {
    if polygon.0.len() > 1 {
        "MultiPolygon"
    } else {
        "Polygon"
    }
}
