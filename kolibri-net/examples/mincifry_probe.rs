//! TLS handshake probe against api2.oneme.ru (Минцифры-signed chain): off should
//! fail on the cert, on should verify.
//!
//!     cargo run --example mincifry_probe

use kolibri_net::{set_trust_mincifry_ca, Client, ClientConfig};

async fn probe(label: &str) -> Result<(), String> {
    let cfg = ClientConfig::new("api2.oneme.ru", 443);
    match Client::connect(cfg).await {
        Ok(_) => {
            println!("[{label}] TLS handshake OK — cert verified");
            Ok(())
        }
        Err(e) => {
            println!("[{label}] failed: {e}");
            Err(e.to_string())
        }
    }
}

#[tokio::main]
async fn main() {
    set_trust_mincifry_ca(false);
    println!("== flag OFF (expect cert failure) ==");
    let off = probe("off").await;

    set_trust_mincifry_ca(true);
    println!("== flag ON (expect success) ==");
    let on = probe("on").await;

    println!("\nresult: off={:?}, on={:?}", off.is_err(), on.is_ok());
    assert!(off.is_err(), "expected verification to FAIL without the CA");
    assert!(on.is_ok(), "expected verification to SUCCEED with the CA");
    println!("PASS: Минцифры CA is exactly what lets the handshake verify");
}
