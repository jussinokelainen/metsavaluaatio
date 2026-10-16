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
    let property_id = normalize_property_id(property_id)?;

    let url = format!("{MML_BASE_URL}/collections/PalstanSijaintitiedot/items");
    let params = [
        ("kiinteistotunnus", property_id.as_str()),
        ("limit", "1000"),
        ("api-key", api_key),
    ];

    let value = http::get_json(&url, &params, 60)?;

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

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
