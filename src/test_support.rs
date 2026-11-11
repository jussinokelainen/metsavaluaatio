//! Yhteiset apufunktiot yksikkötesteille.
//!
//! Käyttää järjestelmän väliaikaiskansiota eikä lisää riippuvuuksia.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Poistettava väliaikishakemisto testiä varten.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Luo uuden yksilöidyn hakemiston annetulla nimellä.
    pub fn new(label: &str) -> Self {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "metsaval_test_{}_{}_{}",
            label,
            std::process::id(),
            counter
        ));
        fs::create_dir_all(&path).expect("väliaikaiskansion luonti epäonnistui");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Polku uuteen tiedostoon hakemiston sisällä.
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Käynnistää yhden pyynnön palvelevan paikallisen HTTP-palvelimen ja
/// palauttaa sen osoitteen sekä kaapatun istuntotekstin.
///
/// Käytetään verkkokoodin testaamiseen ilman oikeaa rajapintaa: palvelin
/// kuuntelee vain loopback-osoitetta (`127.0.0.1`).
pub fn serve_once(status: &str, content_type: &str, body: &str) -> (String, Arc<Mutex<String>>) {
    serve_bytes(status, content_type, body.as_bytes())
}

/// Samanlainen kuin `serve_once`, mutta binääriselle sisällölle (esim. zip).
pub fn serve_bytes(status: &str, content_type: &str, body: &[u8]) -> (String, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("portin avaaminen epäonnistui");
    let address = listener.local_addr().unwrap();
    let request_log = Arc::new(Mutex::new(String::new()));
    let logged = Arc::clone(&request_log);

    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = head.into_bytes();
    response.extend_from_slice(body);

    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let request = read_request(&mut stream);
        *logged.lock().unwrap() = request;
        let _ = stream.write_all(&response);
        let _ = stream.flush();
    });

    (format!("http://{address}"), request_log)
}

/// Lukee HTTP-pyynnön päätteen asti (rivit + tyhjä rivi) tai välimuistista
/// annetun puskurin.
fn read_request(stream: &mut TcpStream) -> String {
    let mut request = String::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                request.push_str(&String::from_utf8_lossy(&buffer[..count]));
                if request.contains("\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    request
}
