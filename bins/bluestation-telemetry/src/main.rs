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

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use clap::Parser;
use tetra_entities::net_telemetry::TELEMETRY_PROTOCOL_VERSION;
use tracing;

use tungstenite::Message;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

use tetra_entities::net_telemetry::codec::TelemetryCodecJson;

#[derive(Parser)]
#[command(name = "bluestation-telemetry", about = "TETRA telemetry service")]
struct Args {
    /// Listen address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:9001")]
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
    tracing::info!("Loaded {} credential(s) from {}", db.len(), path);
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

    let state = Arc::new(SharedState {
        connected: Mutex::new(HashSet::new()),
        anon_counter: AtomicU64::new(0),
    });

    let tls_config = match (&args.cert, &args.key) {
        (Some(cert_path), Some(key_path)) => Some(build_tls_config(cert_path, key_path)),
        (None, None) => None,
        _ => {
            tracing::error!("Both --cert and --key must be provided for TLS");
            std::process::exit(1);
        }
    };

    let listener = TcpListener::bind(&args.listen).unwrap_or_else(|e| {
        tracing::error!("Failed to bind to {}: {}", args.listen, e);
        std::process::exit(1);
    });

    tracing::info!(
        "Telemetry receiver listening on {}{}{}",
        args.listen,
        if tls_config.is_some() { " (TLS)" } else { "" },
        if auth_db.is_some() { " (Basic Auth)" } else { "" },
    );

    for stream in listener.incoming() {
        match stream {
            Ok(tcp) => {
                let peer = tcp.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".into());
                tracing::info!("Connection from {}", peer);

                let tls_cfg = tls_config.clone();
                let auth = auth_db.clone();
                let st = Arc::clone(&state);
                std::thread::spawn(move || {
                    if let Some(cfg) = tls_cfg {
                        let tls_conn = match rustls::ServerConnection::new(cfg) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("[{}] TLS session init failed: {}", peer, e);
                                return;
                            }
                        };
                        let tls_stream = rustls::StreamOwned::new(tls_conn, tcp);
                        handle_connection(tls_stream, &peer, auth.as_deref(), &st);
                    } else {
                        handle_connection(tcp, &peer, auth.as_deref(), &st);
                    }
                });
            }
            Err(e) => {
                tracing::error!("Accept failed: {}", e);
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
/// Returns the authenticated username on success, or `None` on failure.
fn check_basic_auth(req: &Request, auth_db: &AuthDb) -> Option<String> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    let header = req.headers().get("Authorization").and_then(|v| v.to_str().ok())?;
    let encoded = header.strip_prefix("Basic ")?;

    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let (username, password) = credentials.split_once(':')?;

    let phc_hash = auth_db.get(username)?;
    let parsed_hash = PasswordHash::new(phc_hash).ok()?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .ok()
        .map(|_| username.to_string())
}

/// State shared across all connection threads.
struct SharedState {
    /// usernames of currently connected base stations — enforces one-connection-per-user.
    connected: Mutex<HashSet<String>>,
    /// monotonic counter for anonymous client identifiers.
    anon_counter: AtomicU64,
}

fn handle_connection<S: Read + Write>(stream: S, peer: &str, auth_db: Option<&AuthDb>, state: &SharedState) {
    // We need to communicate the resolved display-name out of the handshake
    // callback.  `tungstenite::accept_hdr` takes an `FnMut`, so we use a
    // shared cell that the callback writes into.
    let display_name: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let dn_inner = Arc::clone(&display_name);

    let callback = |req: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
        // --- authenticate & resolve display name ---
        let name = if let Some(db) = auth_db {
            match check_basic_auth(req, db) {
                Some(username) => username,
                None => {
                    tracing::warn!("[{}] rejected: invalid or missing credentials", peer);
                    let mut err = ErrorResponse::new(Some("401 Unauthorized".to_string()));
                    err.headers_mut()
                        .insert("WWW-Authenticate", "Basic realm=\"bluestation-telemetry\"".parse().unwrap());
                    return Err(err);
                }
            }
        } else {
            // No auth configured — assign an anonymous identifier
            let id = state.anon_counter.fetch_add(1, Ordering::Relaxed);
            format!("anonymous{}", id)
        };

        // --- enforce one-connection-per-username ---
        {
            let mut connected = state.connected.lock().unwrap();
            if connected.contains(&name) {
                tracing::warn!("[{}] rejected: '{}' is already connected", peer, name);
                return Err(ErrorResponse::new(Some(format!("user '{}' is already connected", name))));
            }
            connected.insert(name.clone());
        }

        *dn_inner.lock().unwrap() = Some(name);

        // --- subprotocol check ---
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
            // Clean up the connected set since handshake is failing
            if let Some(ref name) = *dn_inner.lock().unwrap() {
                state.connected.lock().unwrap().remove(name);
            }
            tracing::warn!(
                "[{}] rejected: expected subprotocol '{}', got '{}'",
                peer,
                TELEMETRY_PROTOCOL_VERSION,
                proto
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
            // Clean up connected set if the name was already inserted
            if let Some(ref name) = *display_name.lock().unwrap() {
                state.connected.lock().unwrap().remove(name);
            }
            tracing::error!("[{}] WebSocket handshake failed: {}", peer, e);
            return;
        }
    };

    let name = display_name.lock().unwrap().clone().unwrap_or_else(|| "unknown".to_string());

    let codec = TelemetryCodecJson;
    tracing::info!("[{}] connected (peer {})", name, peer);

    loop {
        match ws.read() {
            Ok(Message::Binary(data)) => match codec.decode(&data) {
                Ok(event) => {
                    tracing::info!("[{}] {:?}", name, event);
                }
                Err(e) => {
                    tracing::error!("[{}] deserialize error: {}", name, e);
                }
            },
            Ok(Message::Text(text)) => {
                tracing::error!("[{}] unexpected text message ({} bytes), expected binary", name, text.len());
            }
            Ok(Message::Ping(_)) => {
                // tungstenite auto-queues a Pong reply; just flush it out.
                if ws.flush().is_err() {
                    tracing::info!("[{}] failed to flush pong, disconnecting", name);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                tracing::info!("[{}] disconnected", name);
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::ConnectionClosed) => {
                tracing::info!("[{}] connection closed", name);
                break;
            }
            Err(tungstenite::Error::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)) => {
                tracing::info!("[{}] connection reset", name);
                break;
            }
            Err(e) => {
                tracing::error!("[{}] read error: {}", name, e);
                break;
            }
        }
    }

    // Release the slot so the same user can reconnect
    state.connected.lock().unwrap().remove(&name);
    tracing::info!("[{}] slot released", name);
}
