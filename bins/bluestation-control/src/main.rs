//! bluestation-telemetry — minimal WebSocket telemetry receiver
//!
//! Listens for incoming WebSocket connections and prints every
//! [`TelemetryEvent`] received, deserialized via the shared codec module.
//!
//! Optional HTTP Basic Auth can be enabled by passing `--auth-file` with a path
//! to a text file containing `username:<argon2-phc-hash>` entries, one per line.
//! Empty lines and lines starting with `#` are ignored.
//!
//! Generate credential lines with `contrib/generate_credential.sh`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

use clap::Parser;
use tetra_entities::net_telemetry::TELEMETRY_PROTOCOL_VERSION;
use tracing::{error, info, warn};
use tungstenite::Message;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

use tetra_entities::net_telemetry::codec::TelemetryCodecJson;

#[derive(Parser)]
#[command(name = "bluestation-telemetry", about = "TETRA telemetry receiver")]
struct Args {
    /// Listen address (host:port)
    #[arg(short, long, default_value = "0.0.0.0:9001")]
    listen: String,

    /// Path to PEM-encoded server certificate chain for TLS
    #[arg(long)]
    cert: Option<String>,

    /// Path to PEM-encoded private key for TLS
    #[arg(long)]
    key: Option<String>,

    /// Path to a text file with `username:<argon2-phc-hash>` entries (one per
    /// line) for HTTP Basic Auth. When omitted, no authentication is required.
    /// Generate entries with `contrib/generate_credential.sh`.
    #[arg(long)]
    auth_file: Option<String>,
    /// Generate a credential line for the auth file and exit.
    /// Reads username and password from stdin.
    #[arg(long)]
    generate_credential: bool,
}

/// Map of username → Argon2 PHC hash string loaded from the auth file.
type AuthDb = HashMap<String, String>;

/// Load credentials from a text file. Each non-empty, non-comment line must be
/// formatted as `username:$argon2id$...` (PHC string format).
fn load_auth_db(path: &str) -> AuthDb {
    use argon2::PasswordHash;

    let file = std::fs::File::open(path).unwrap_or_else(|e| {
        eprintln!("Failed to open auth file '{}': {}", path, e);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);
    let mut db = HashMap::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.unwrap_or_else(|e| {
            eprintln!("Failed to read line {} of '{}': {}", i + 1, path, e);
            std::process::exit(1);
        });
        let line = line.trim().to_string();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Split on first ':' only — the PHC hash string contains '$' but no ':'
        let (username, phc_hash) = match line.split_once(':') {
            Some((u, h)) => (u.trim(), h.trim()),
            None => {
                eprintln!("Auth file '{}' line {}: expected 'username:$argon2id$...' format", path, i + 1);
                std::process::exit(1);
            }
        };
        // Validate the PHC hash string at load time so we fail fast
        if PasswordHash::new(phc_hash).is_err() {
            eprintln!(
                "Auth file '{}' line {}: invalid Argon2 PHC hash for user '{}'",
                path,
                i + 1,
                username
            );
            std::process::exit(1);
        }
        db.insert(username.to_string(), phc_hash.to_string());
    }
    if db.is_empty() {
        eprintln!("Auth file '{}' contains no credentials", path);
        std::process::exit(1);
    }
    info!("Loaded {} credential(s) from {}", db.len(), path);
    db
}

/// Prompt for username and password, then print an auth-file line to stdout.
fn generate_credential() {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };

    eprint!("Username: ");
    let mut username = String::new();
    std::io::stdin().read_line(&mut username).unwrap_or_else(|e| {
        eprintln!("Failed to read username: {}", e);
        std::process::exit(1);
    });
    let username = username.trim();
    if username.is_empty() || username.contains(':') {
        eprintln!("Username must be non-empty and must not contain ':'");
        std::process::exit(1);
    }

    eprint!("Password: ");
    let mut password = String::new();
    std::io::stdin().read_line(&mut password).unwrap_or_else(|e| {
        eprintln!("Failed to read password: {}", e);
        std::process::exit(1);
    });
    let password = password.trim();
    if password.is_empty() {
        eprintln!("Password must be non-empty");
        std::process::exit(1);
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap_or_else(|e| {
        eprintln!("Failed to hash password: {}", e);
        std::process::exit(1);
    });

    println!("{}:{}", username, hash);
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    if args.generate_credential {
        generate_credential();
        return;
    }

    let auth_db: Option<Arc<AuthDb>> = args.auth_file.as_deref().map(|path| Arc::new(load_auth_db(path)));

    let tls_config = match (&args.cert, &args.key) {
        (Some(cert_path), Some(key_path)) => Some(build_tls_config(cert_path, key_path)),
        (None, None) => None,
        _ => {
            error!("Both --cert and --key must be provided for TLS");
            std::process::exit(1);
        }
    };

    let listener = TcpListener::bind(&args.listen).unwrap_or_else(|e| {
        error!("Failed to bind to {}: {}", args.listen, e);
        std::process::exit(1);
    });

    info!(
        "Telemetry receiver listening on {}{}{}",
        args.listen,
        if tls_config.is_some() { " (TLS)" } else { "" },
        if auth_db.is_some() { " (Basic Auth)" } else { "" },
    );

    for stream in listener.incoming() {
        match stream {
            Ok(tcp) => {
                let peer = tcp.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".into());
                info!("Connection from {}", peer);

                let tls_cfg = tls_config.clone();
                let auth = auth_db.clone();
                std::thread::spawn(move || {
                    if let Some(cfg) = tls_cfg {
                        let tls_conn = match rustls::ServerConnection::new(cfg) {
                            Ok(c) => c,
                            Err(e) => {
                                error!("[{}] TLS session init failed: {}", peer, e);
                                return;
                            }
                        };
                        let tls_stream = rustls::StreamOwned::new(tls_conn, tcp);
                        handle_connection(tls_stream, &peer, auth.as_deref());
                    } else {
                        handle_connection(tcp, &peer, auth.as_deref());
                    }
                });
            }
            Err(e) => {
                error!("Accept failed: {}", e);
            }
        }
    }
}

fn build_tls_config(cert_path: &str, key_path: &str) -> Arc<rustls::ServerConfig> {
    let cert_file = std::fs::File::open(cert_path).unwrap_or_else(|e| {
        eprintln!("Failed to open cert file '{}': {}", cert_path, e);
        std::process::exit(1);
    });
    let key_file = std::fs::File::open(key_path).unwrap_or_else(|e| {
        eprintln!("Failed to open key file '{}': {}", key_path, e);
        std::process::exit(1);
    });

    let certs: Vec<_> = rustls_pemfile::certs(&mut BufReader::new(cert_file))
        .collect::<Result<_, _>>()
        .unwrap_or_else(|e| {
            eprintln!("Failed to parse cert PEM '{}': {}", cert_path, e);
            std::process::exit(1);
        });

    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .unwrap_or_else(|e| {
            eprintln!("Failed to parse key PEM '{}': {}", key_path, e);
            std::process::exit(1);
        })
        .unwrap_or_else(|| {
            eprintln!("No private key found in '{}'", key_path);
            std::process::exit(1);
        });

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap_or_else(|e| {
            eprintln!("Invalid TLS configuration: {}", e);
            std::process::exit(1);
        });

    Arc::new(config)
}

/// Validate HTTP Basic Auth credentials from the `Authorization` header.
/// Decodes the Basic Auth value, looks up the username in the auth DB, and
/// verifies the password against the stored Argon2 PHC hash.
fn check_basic_auth(req: &Request, auth_db: Option<&AuthDb>) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    let db = match auth_db {
        Some(db) => db,
        None => return true, // No auth required
    };

    let header = match req.headers().get("Authorization").and_then(|v| v.to_str().ok()) {
        Some(h) => h,
        None => return false,
    };

    let encoded = match header.strip_prefix("Basic ") {
        Some(e) => e,
        None => return false,
    };

    use base64::Engine;
    let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded) {
        Ok(d) => d,
        Err(_) => return false,
    };

    let credentials = match String::from_utf8(decoded) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let (username, password) = match credentials.split_once(':') {
        Some(pair) => pair,
        None => return false,
    };

    let phc_hash = match db.get(username) {
        Some(h) => h,
        None => return false,
    };

    let parsed_hash = match PasswordHash::new(phc_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
}

fn handle_connection<S: Read + Write>(stream: S, peer: &str, auth_db: Option<&AuthDb>) {
    let callback = |req: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
        // Verify HTTP Basic Auth if enabled
        if !check_basic_auth(req, auth_db) {
            warn!("[{}] Rejected: invalid or missing Basic Auth credentials", peer);
            let mut err = ErrorResponse::new(Some("401 Unauthorized".to_string()));
            err.headers_mut()
                .insert("WWW-Authenticate", "Basic realm=\"bluestation-telemetry\"".parse().unwrap());
            return Err(err);
        }

        let proto = req
            .headers()
            .get("Sec-WebSocket-Protocol")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if proto.split(',').map(|s| s.trim()).any(|s| s == TELEMETRY_PROTOCOL_VERSION) {
            response
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", TELEMETRY_PROTOCOL_VERSION.parse().unwrap());
            Ok(response)
        } else {
            warn!(
                "[{}] Rejected: expected subprotocol '{}', got '{}'",
                peer, TELEMETRY_PROTOCOL_VERSION, proto
            );
            Err(ErrorResponse::new(Some(format!(
                "unsupported subprotocol; expected {}",
                TELEMETRY_PROTOCOL_VERSION
            ))))
        }
    };

    let mut ws = match tungstenite::accept_hdr(stream, callback) {
        Ok(ws) => ws,
        Err(e) => {
            error!("[{}] WebSocket handshake failed: {}", peer, e);
            return;
        }
    };

    let codec = TelemetryCodecJson;
    info!("[{}] WebSocket connected", peer);

    loop {
        match ws.read() {
            Ok(Message::Binary(data)) => match codec.decode(&data) {
                Ok(event) => {
                    println!("{:?}", event);
                }
                Err(e) => {
                    error!("[{}] Deserialize error: {}", peer, e);
                }
            },
            Ok(Message::Text(text)) => {
                error!("[{}] Unexpected text message ({} bytes), expected binary", peer, text.len());
            }
            Ok(Message::Ping(_)) => {
                // tungstenite auto-queues a Pong reply; just flush it out.
                if ws.flush().is_err() {
                    info!("[{}] Failed to flush pong, disconnecting", peer);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                info!("[{}] Client disconnected", peer);
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::ConnectionClosed) => {
                info!("[{}] Connection closed", peer);
                break;
            }
            Err(tungstenite::Error::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)) => {
                info!("[{}] Connection reset", peer);
                break;
            }
            Err(e) => {
                error!("[{}] Read error: {}", peer, e);
                break;
            }
        }
    }
}
