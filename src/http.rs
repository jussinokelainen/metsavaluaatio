use color_eyre::Result;
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::Duration;

/// Tekee GET-pyynnön ja palauttaa vastauksen. Tulostaa virhetilanteessa
/// palvelimen vastauksen, kuten alkuperäinen Python-skripti.
///
/// Käyttää `reqwest`:in asynkronista API:a, joten pyyntö odottaa vain
/// vastausta eikä se blokkaa asynkronista runtimea.
async fn get(url: &str, params: &[(&str, &str)], timeout_secs: u64) -> Result<Response> {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?;

    let response = client.get(url).query(params).send().await?;

    if !response.status().is_success() {
        println!("{}", "=".repeat(70));
        println!("HTTP ERROR");
        println!("{}", "=".repeat(70));
        println!("Status: {}", response.status());
        println!("URL: {}", response.url());

        let body = response.text().await.unwrap_or_default();
        println!("\nServer response:");
        println!("{}", &body[..body.len().min(5000)]);
        println!("{}", "=".repeat(70));

        color_eyre::eyre::bail!("HTTP-pyyntö epäonnistui");
    }

    Ok(response)
}

/// Hakee ja jäsentää JSON-vastauksen.
pub async fn get_json(url: &str, params: &[(&str, &str)], timeout_secs: u64) -> Result<Value> {
    let response = get(url, params, timeout_secs).await?;
    Ok(response.json().await?)
}

/// Hakee raakadatan (esim. GeoJSON tai ZIP) tavuina.
pub async fn get_bytes(url: &str, params: &[(&str, &str)], timeout_secs: u64) -> Result<Vec<u8>> {
    let response = get(url, params, timeout_secs).await?;
    Ok(response.bytes().await?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::serve_once;
    use serde_json::json;
    use std::net::TcpListener;

    #[tokio::test]
    async fn get_json_returns_the_parsed_body() {
        let (url, _log) = serve_once("200 OK", "application/json", r#"{"features":[1,2]}"#);
        let value = get_json(&url, &[("foo", "bar")], 5).await.unwrap();
        assert_eq!(value, json!({"features": [1, 2]}));
    }

    #[tokio::test]
    async fn get_bytes_returns_the_raw_body() {
        let (url, _log) = serve_once("200 OK", "application/octet-stream", "PK\x03\x04data");
        let bytes = get_bytes(&url, &[], 5).await.unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        assert_eq!(&bytes[4..], b"data");
    }

    #[tokio::test]
    async fn query_parameters_are_sent_in_the_request_line() {
        let (url, log) = serve_once("200 OK", "application/json", "{}");
        get_json(
            &url,
            &[("kiinteistotunnus", "01234567890123"), ("limit", "1000")],
            5,
        )
        .await
        .unwrap();

        let request = log.lock().unwrap().clone();
        let first_line = request.lines().next().unwrap().to_string();
        assert!(
            first_line.starts_with("GET /?"),
            "pyyntölinja: {first_line}"
        );
        assert!(first_line.contains("kiinteistotunnus=01234567890123"));
        assert!(first_line.contains("limit=1000"));
        assert!(request.to_lowercase().contains("host:"));
    }

    #[tokio::test]
    async fn error_status_is_reported_as_an_error() {
        let (url, _log) = serve_once(
            "500 Internal Server Error",
            "text/plain",
            "palvelin meni poks",
        );
        assert!(get_json(&url, &[], 5).await.is_err());
    }

    #[tokio::test]
    async fn not_found_is_reported_as_an_error() {
        let (url, _log) = serve_once("404 Not Found", "text/plain", "ei ole täällä");
        assert!(get_bytes(&url, &[], 5).await.is_err());
    }

    #[tokio::test]
    async fn invalid_json_body_is_an_error() {
        let (url, _log) = serve_once("200 OK", "application/json", "tätä ei ole jsonia");
        assert!(get_json(&url, &[], 5).await.is_err());
    }

    #[tokio::test]
    async fn unreachable_server_is_an_error() {
        // Varattu portti ilman kuuntelijaa
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        assert!(
            get_json(&format!("http://{address}"), &[], 5)
                .await
                .is_err()
        );
    }
}
