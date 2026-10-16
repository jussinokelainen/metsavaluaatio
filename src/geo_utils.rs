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

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{LineString, Polygon};
    use serde_json::json;

    fn square_polygon(min: f64, size: f64) -> Polygon<f64> {
        let max = min + size;
        Polygon::new(
            LineString::from(vec![
                (min, min),
                (max, min),
                (max, max),
                (min, max),
                (min, min),
            ]),
            vec![],
        )
    }

    fn square(min: f64, size: f64) -> MultiPolygon<f64> {
        MultiPolygon::new(vec![square_polygon(min, size)])
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn value_i64_accepts_numbers_and_numeric_strings() {
        assert_eq!(value_i64(&json!(42)), Some(42));
        assert_eq!(value_i64(&json!(42.0)), Some(42));
        assert_eq!(value_i64(&json!("42")), Some(42));
        assert_eq!(value_i64(&json!(" 42 ")), Some(42));
    }

    #[test]
    fn value_i64_rejects_non_numeric_values() {
        assert_eq!(value_i64(&json!("abc")), None);
        assert_eq!(value_i64(&json!(null)), None);
        assert_eq!(value_i64(&json!(true)), None);
    }

    #[test]
    fn value_f64_accepts_numbers_and_numeric_strings() {
        assert_eq!(value_f64(&json!(1.5)), Some(1.5));
        assert_eq!(value_f64(&json!("2.5")), Some(2.5));
        assert_eq!(value_f64(&json!(" 2.5 ")), Some(2.5));
        assert_eq!(value_f64(&json!(true)), None);
    }

    #[test]
    fn bounds_wrap_the_whole_polygon() {
        assert_eq!(bounds(&square(0.0, 10.0)), Some((0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn area_is_measured_in_square_metres() {
        assert_close(area_m2(&square(0.0, 10.0)), 100.0);
    }

    #[test]
    fn geometry_type_distinguishes_single_and_multi() {
        assert_eq!(geometry_type(&square(0.0, 1.0)), "Polygon");
        let multi = MultiPolygon::new(vec![square_polygon(0.0, 1.0), square_polygon(5.0, 1.0)]);
        assert_eq!(geometry_type(&multi), "MultiPolygon");
    }

    fn clipped_feature() -> geojson::Feature {
        let feature = json!({
            "type": "Feature",
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]]]
            },
            "properties": {
                "standid": 7,
                "standnumber": "3",
                "cuttingtype": 5,
                "area": 1.5
            }
        });
        serde_json::from_value(feature).unwrap()
    }

    #[test]
    fn clip_stand_keeps_properties_and_measures_the_intersection() {
        let stand = clip_stand(&clipped_feature(), &square(0.0, 10.0)).unwrap();
        assert_eq!(stand.standid, Some(7));
        assert_eq!(stand.standnumber, Some(3));
        assert_eq!(stand.cuttingtype, Some(5));
        assert_close(stand.intersection_area_m2, 100.0);
    }

    #[test]
    fn clip_stand_skips_intersections_below_the_threshold() {
        let feature = json!({
            "type": "Feature",
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5], [0.0, 0.0]]]
            }
        });
        let feature = serde_json::from_value(feature).unwrap();
        assert!(clip_stand(&feature, &square(0.0, 10.0)).is_none());
    }
}
