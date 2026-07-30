use color_eyre::Result;
use reqwest::blocking::{Client, Response};
use serde_json::Value;
use std::time::Duration;

/// Tekee GET-pyynnön ja palauttaa vastauksen. Tulostaa virhetilanteessa
/// palvelimen vastauksen, kuten alkuperäinen Python-skripti.
fn get(url: &str, params: &[(&str, &str)], timeout_secs: u64) -> Result<Response> {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?;

    let response = client.get(url).query(params).send()?;

    if !response.status().is_success() {
        println!("{}", "=".repeat(70));
        println!("HTTP ERROR");
        println!("{}", "=".repeat(70));
        println!("Status: {}", response.status());
        println!("URL: {}", response.url());

        let body = response.text().unwrap_or_default();
        println!("\nServer response:");
        println!("{}", &body[..body.len().min(5000)]);
        println!("{}", "=".repeat(70));

        color_eyre::eyre::bail!("HTTP-pyyntö epäonnistui");
    }

    Ok(response)
}

/// Hakee ja jäsentää JSON-vastauksen.
pub fn get_json(url: &str, params: &[(&str, &str)], timeout_secs: u64) -> Result<Value> {
    let response = get(url, params, timeout_secs)?;
    Ok(response.json()?)
}

/// Hakee raakadatan (esim. GeoJSON tai ZIP) tavuina.
pub fn get_bytes(url: &str, params: &[(&str, &str)], timeout_secs: u64) -> Result<Vec<u8>> {
    let response = get(url, params, timeout_secs)?;
    Ok(response.bytes()?.to_vec())
}
