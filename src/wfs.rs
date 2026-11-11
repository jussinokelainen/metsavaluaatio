use crate::http;
use color_eyre::Result;
use geojson::Feature;
use minidom::Element;
use serde_json::Value;

/// Yksi WFS-rajapinnan karttataso.
pub struct Layer {
    pub name: String,
    pub title: String,
}

/// Hakee WFS GetCapabilities -vastauksen.
pub fn get_capabilities(wfs_url: &str) -> Result<Vec<u8>> {
    let params = [
        ("service", "WFS"),
        ("version", "2.0.0"),
        ("request", "GetCapabilities"),
    ];

    http::get_bytes(wfs_url, &params, 120)
}

/// Listaa WFS-rajapinnan karttatasot.
pub fn get_layers(wfs_url: &str) -> Result<Vec<Layer>> {
    let content = get_capabilities(wfs_url)?;
    let text = std::str::from_utf8(&content)?;
    let root: Element = text.parse()?;

    let mut layers = Vec::new();
    collect_feature_types(&root, &mut layers);

    Ok(layers)
}

fn collect_feature_types(element: &Element, out: &mut Vec<Layer>) {
    for child in element.children() {
        if child.name() == "FeatureType" {
            if let Some(name) = child_text(child, "Name") {
                out.push(Layer {
                    title: child_text(child, "Title").unwrap_or_default(),
                    name,
                });
            }
        }

        collect_feature_types(child, out);
    }
}

fn child_text(element: &Element, name: &str) -> Option<String> {
    element
        .children()
        .find(|child| child.name() == name)
        .map(|child| child.text())
}

/// Tunnistaa metsävaratason WFS:n karttatasoista.
pub fn select_stand_layer(layers: &[Layer]) -> Result<String> {
    if layers.len() == 1 {
        return Ok(layers[0].name.clone());
    }

    let candidates: Vec<&Layer> = layers
        .iter()
        .filter(|layer| {
            let name = layer.name.to_lowercase();
            let title = layer.title.to_lowercase();
            name.contains("stand") || title.contains("metsävara") || title.contains("stand")
        })
        .collect();

    match candidates.len() {
        1 => Ok(candidates[0].name.clone()),
        0 => {
            for layer in layers {
                println!("{}  ({})", layer.name, layer.title);
            }
            color_eyre::eyre::bail!("Metsävaralayeria ei tunnistettu.");
        }
        _ => {
            println!("Useita mahdollisia metsävaralayereita:");
            for layer in &candidates {
                println!("{}  ({})", layer.name, layer.title);
            }
            color_eyre::eyre::bail!("Valitse STAND_LAYER yllä olevasta listasta.");
        }
    }
}

/// Hakee WFS-kuviot annetulla rajauslaatikolla (bbox).
pub fn get_features_bbox(
    wfs_url: &str,
    layer: &str,
    bbox: &str,
    srs: &str,
    max_features: u32,
) -> Result<Vec<Feature>> {
    let max_features = max_features.to_string();
    let params = [
        ("service", "WFS"),
        ("version", "2.0.0"),
        ("request", "GetFeature"),
        ("typeNames", layer),
        ("outputFormat", "application/json"),
        ("bbox", bbox),
        ("srsName", srs),
        ("maxFeatures", max_features.as_str()),
    ];

    let value = http::get_json(wfs_url, &params, 180)?;
    Ok(extract_features(value))
}

fn extract_features(value: Value) -> Vec<Feature> {
    value
        .get("features")
        .and_then(|features| features.as_array())
        .map(|features| {
            features
                .iter()
                .filter_map(|feature| serde_json::from_value::<Feature>(feature.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::serve_once;
    use serde_json::json;

    fn layer(name: &str, title: &str) -> Layer {
        Layer {
            name: name.to_string(),
            title: title.to_string(),
        }
    }

    #[test]
    fn single_layer_is_selected_automatically() {
        let layers = vec![layer("mv:foo", "Foo")];
        assert_eq!(select_stand_layer(&layers).unwrap(), "mv:foo");
    }

    #[test]
    fn layer_with_stand_in_name_is_selected() {
        let layers = vec![
            layer("mv:kanta_alueet", "Kanta-alueet"),
            layer("mv:metsavarat_stand", "Metsävarat"),
        ];
        assert_eq!(select_stand_layer(&layers).unwrap(), "mv:metsavarat_stand");
    }

    #[test]
    fn layer_with_metsavara_in_title_is_selected() {
        let layers = vec![
            layer("mv:kanta", "Kanta-alueet"),
            layer("mv:tk2", "Metsävarakuvion kuvio- ja puustotiedot"),
        ];
        assert_eq!(select_stand_layer(&layers).unwrap(), "mv:tk2");
    }

    #[test]
    fn missing_stand_layer_is_an_error() {
        let layers = vec![layer("mv:a", "A"), layer("mv:b", "B")];
        assert!(select_stand_layer(&layers).is_err());
    }

    #[test]
    fn ambiguous_stand_layers_are_an_error() {
        let layers = vec![layer("stand:one", "One"), layer("stand:two", "Two")];
        assert!(select_stand_layer(&layers).is_err());
    }

    #[test]
    fn no_layers_at_all_is_an_error() {
        assert!(select_stand_layer(&[]).is_err());
    }

    #[test]
    fn extract_features_parses_valid_features() {
        let value = json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [1.0, 2.0]},
                    "properties": {"standid": 7}
                },
                {"type": "EiGeoJSONa"},
                42
            ]
        });

        let features = extract_features(value);
        assert_eq!(features.len(), 1);
        assert_eq!(
            features[0].properties.as_ref().unwrap().get("standid"),
            Some(&json!(7))
        );
    }

    #[test]
    fn extract_features_tolerates_missing_or_malformed_payloads() {
        assert!(extract_features(json!({})).is_empty());
        assert!(extract_features(json!({"features": "not an array"})).is_empty());
        assert!(extract_features(json!({"features": null})).is_empty());
        assert!(extract_features(Value::Null).is_empty());
    }

    const CAPABILITIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<WFS_Capabilities xmlns="http://www.opengis.net/wfs/2.0" version="2.0.0">
    <FeatureTypeList>
        <FeatureType>
            <Name>mv:tk2</Name>
            <Title>Metsävarakuvion kuvio- ja puustotiedot</Title>
        </FeatureType>
        <FeatureType>
            <Name>mv:kanta</Name>
            <Title>Kanta-alueet</Title>
        </FeatureType>
    </FeatureTypeList>
    <OperationsMetadata>
        <Operation name="GetCapabilities"/>
    </OperationsMetadata>
</WFS_Capabilities>"#;

    #[test]
    fn feature_types_are_collected_from_capabilities() {
        let root: Element = CAPABILITIES.parse().unwrap();
        let mut layers = Vec::new();
        collect_feature_types(&root, &mut layers);

        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].name, "mv:tk2");
        assert_eq!(layers[0].title, "Metsävarakuvion kuvio- ja puustotiedot");
        assert_eq!(layers[1].name, "mv:kanta");
        assert_eq!(layers[1].title, "Kanta-alueet");
    }

    #[test]
    fn feature_type_without_a_name_is_skipped() {
        let xml = r#"<root xmlns="urn:test">
            <FeatureType><Title>Nimetön</Title></FeatureType>
            <FeatureType><Name>mv:nimi</Name></FeatureType>
        </root>"#;
        let root: Element = xml.parse().unwrap();
        let mut layers = Vec::new();
        collect_feature_types(&root, &mut layers);

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].name, "mv:nimi");
        assert_eq!(layers[0].title, "");
    }

    #[test]
    fn child_text_returns_none_for_missing_elements() {
        let root: Element = CAPABILITIES.parse().unwrap();
        assert!(child_text(&root, "FeatureTypeList").is_some());
        assert_eq!(child_text(&root, "EiOlemassa"), None);
    }

    #[test]
    fn malformed_capabilities_xml_is_an_error() {
        let root = "<WFS_Capabilities><unclosed>";
        assert!(root.parse::<Element>().is_err());
    }

    #[test]
    fn get_capabilities_fetches_the_document_with_wfs_parameters() {
        let (url, log) = serve_once("200 OK", "application/xml", CAPABILITIES);

        let content = get_capabilities(&url).unwrap();
        assert_eq!(std::str::from_utf8(&content).unwrap(), CAPABILITIES);

        let request = log.lock().unwrap().clone();
        let first_line = request.lines().next().unwrap().to_string();
        assert!(
            first_line.starts_with("GET /?"),
            "pyyntölinja: {first_line}"
        );
        assert!(first_line.contains("service=WFS"));
        assert!(first_line.contains("version=2.0.0"));
        assert!(first_line.contains("request=GetCapabilities"));
    }

    #[test]
    fn get_layers_fetches_and_parses_the_document() {
        let (url, _log) = serve_once("200 OK", "application/xml", CAPABILITIES);

        let layers = get_layers(&url).unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].name, "mv:tk2");
        assert_eq!(layers[1].name, "mv:kanta");
    }

    #[test]
    fn get_layers_rejects_malformed_xml() {
        let (url, _log) = serve_once("200 OK", "application/xml", "<WFS_Capabilities><unclosed>");
        assert!(get_layers(&url).is_err());
    }

    #[test]
    fn get_layers_reports_http_errors() {
        let (url, _log) = serve_once("500 Internal Server Error", "text/plain", "poks");
        assert!(get_layers(&url).is_err());
    }

    #[test]
    fn get_features_bbox_sends_the_query_and_returns_features() {
        let body = json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Polygon", "coordinates": [[[24.0, 60.0], [24.1, 60.0], [24.1, 60.1], [24.0, 60.0]]]},
                    "properties": {"standid": 42}
                }
            ]
        })
        .to_string();
        let (url, log) = serve_once("200 OK", "application/json", &body);

        let features = get_features_bbox(
            &url,
            "mv:tk2",
            "385000,6700000,385100,6700100",
            "EPSG:3067",
            10_000,
        )
        .unwrap();
        assert_eq!(features.len(), 1);
        assert_eq!(
            features[0].properties.as_ref().unwrap().get("standid"),
            Some(&json!(42))
        );

        let request = log.lock().unwrap().clone();
        let first_line = request.lines().next().unwrap().to_string();
        assert!(first_line.contains("request=GetFeature"));
        assert!(
            first_line.contains("typeNames=mv%3Atk2") || first_line.contains("typeNames=mv:tk2")
        );
        assert!(first_line.contains("bbox=385000%2C6700000%2C385100%2C6700100"));
        assert!(first_line.contains("srsName=EPSG%3A3067"));
        assert!(first_line.contains("maxFeatures=10000"));
    }

    #[test]
    fn get_features_bbox_tolerates_a_payload_without_features() {
        let (url, _log) = serve_once("200 OK", "application/json", "{}");
        let features = get_features_bbox(&url, "mv:tk2", "1,2,3,4", "EPSG:3067", 10).unwrap();
        assert!(features.is_empty());
    }
}
