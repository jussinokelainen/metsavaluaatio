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
