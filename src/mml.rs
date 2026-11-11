use crate::consts::MML_BASE_URL;
use crate::http;
use color_eyre::Result;
use geojson::Feature;

/// Poistaa kiinteistötunnuksesta ylimääräiset merkit ja varmistaa, että
/// numeroita on 14.
///
/// Esim. `740-555-0002-0000` -> `74055500020000`
pub fn normalize_property_id(property_id: &str) -> Result<String> {
    let property_id = property_id.trim();
    let digits: String = property_id.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() != 14 {
        color_eyre::eyre::bail!(
            "Virheellinen kiinteistötunnus: {property_id}\n\
             Numeroita löytyi {}, mutta pitäisi löytyä 14.",
            digits.len()
        );
    }

    Ok(digits)
}

/// Hakee kaikki kiinteistötunnukseen liittyvät palstat MML:stä.
pub fn get_property_features(property_id: &str, api_key: &str) -> Result<Vec<Feature>> {
    let url = format!("{MML_BASE_URL}/collections/PalstanSijaintitiedot/items");
    fetch_property_features(&url, property_id, api_key)
}

/// Hakee palstat annetusta osoitteesta. Toteuttaa `get_property_features`
/// logiikan; osoitteen erottaminen mahdollistaa rajapinnan testauksen.
fn fetch_property_features(url: &str, property_id: &str, api_key: &str) -> Result<Vec<Feature>> {
    let property_id = normalize_property_id(property_id)?;

    let params = [
        ("kiinteistotunnus", property_id.as_str()),
        ("limit", "1000"),
        ("api-key", api_key),
    ];

    let value = http::get_json(url, &params, 60)?;

    Ok(parse_property_features(value))
}

/// Jäsentää MML:n GeoJSON-vastauksen palstalistaksi. Virheelliset palstat
/// ohitetaan.
fn parse_property_features(value: serde_json::Value) -> Vec<Feature> {
    let features = value
        .get("features")
        .and_then(|features| features.as_array())
        .cloned()
        .unwrap_or_default();

    let mut result = Vec::new();
    for feature in features {
        if let Ok(feature) = serde_json::from_value::<Feature>(feature) {
            result.push(feature);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::serve_once;
    use serde_json::json;

    #[test]
    fn normalize_strips_separators() {
        let normalized = normalize_property_id("740-555-0002-0000").unwrap();
        assert_eq!(normalized, "74055500020000");
    }

    #[test]
    fn normalize_trims_surrounding_whitespace() {
        let normalized = normalize_property_id("  740-555-0002-0000  ").unwrap();
        assert_eq!(normalized, "74055500020000");
    }

    #[test]
    fn normalize_rejects_wrong_digit_count() {
        assert!(normalize_property_id("123").is_err());
        assert!(normalize_property_id("740-555-0002-00001").is_err());
        assert!(normalize_property_id("").is_err());
    }

    #[test]
    fn parse_property_features_keeps_valid_features() {
        let value = json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Polygon", "coordinates": [[[24.0, 60.0], [24.1, 60.0], [24.1, 60.1], [24.0, 60.0]]]},
                    "properties": {"kiinteistotunnus": "74055500020000"}
                },
                {"type": "EiGeoJSONa"},
                42,
                null
            ]
        });

        let features = parse_property_features(value);
        assert_eq!(features.len(), 1);
        let props = features[0].properties.as_ref().unwrap();
        assert_eq!(
            props.get("kiinteistotunnus"),
            Some(&json!("74055500020000"))
        );
    }

    #[test]
    fn parse_property_features_tolerates_missing_payloads() {
        assert!(parse_property_features(json!({})).is_empty());
        assert!(parse_property_features(json!({"features": "not an array"})).is_empty());
        assert!(parse_property_features(json!({"features": null})).is_empty());
        assert!(parse_property_features(serde_json::Value::Null).is_empty());
    }

    #[test]
    fn fetch_property_features_requests_the_given_endpoint() {
        let body = json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [24.0, 60.0]},
                    "properties": {}
                }
            ]
        })
        .to_string();
        let (url, log) = serve_once("200 OK", "application/json", &body);

        let features = fetch_property_features(&url, "740-555-0002-0000", "avain").unwrap();
        assert_eq!(features.len(), 1);

        let request = log.lock().unwrap().clone();
        let first_line = request.lines().next().unwrap().to_string();
        assert!(
            first_line.starts_with("GET /?"),
            "pyyntölinja: {first_line}"
        );
        assert!(first_line.contains("kiinteistotunnus=74055500020000"));
        assert!(first_line.contains("limit=1000"));
        assert!(first_line.contains("api-key=avain"));
    }

    #[test]
    fn fetch_property_features_reports_http_errors() {
        let (url, _log) = serve_once("500 Internal Server Error", "text/plain", "virhe");
        assert!(fetch_property_features(&url, "740-555-0002-0000", "avain").is_err());
    }

    #[test]
    fn fetch_property_features_rejects_a_malformed_property_id() {
        let (url, _log) = serve_once("200 OK", "application/json", "{}");
        assert!(fetch_property_features(&url, "ei-tunnus", "avain").is_err());
    }

    #[test]
    fn get_property_features_rejects_a_malformed_property_id_before_any_request() {
        // Virheellinen tunnus hylätään ennen kuin mitään pyyntöä tehdään,
        // joten testi ei tarvitse yhteyttä oikeaan MML-rajapintaan
        assert!(get_property_features("ei-tunnus", "avain").is_err());
    }
}
