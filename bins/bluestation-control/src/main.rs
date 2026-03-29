//! bluestation-control — minimal WebSocket control receiver
//!
//! Listens for incoming WebSocket connections and prints every
//! [`ControlEvent`] received, deserialized via the shared codec module.
//!
//! Optional HTTP Basic Auth can be enabled by passing `--auth-file` with a path
//! to a text file containing `username:<argon2-phc-hash>` entries, one per line.
//! Empty lines and lines starting with `#` are ignored.
//!
//! Generate credential lines with `contrib/generate_credential.sh`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use clap::Parser;
use crossbeam_channel::{Receiver, Sender, unbounded};
use tetra_entities::net_control::CONTROL_PROTOCOL_VERSION;
use tetra_entities::net_control::codec::ControlCodecJson;
use tetra_entities::net_control::commands::ControlCommand;
use tracing::{error, info, warn};
use tungstenite::Message;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

#[derive(Parser)]
#[command(name = "bluestation-control", about = "TETRA control service")]
struct Args {
    /// Listen address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:9002")]
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

/// Shared registry of connected base-station command senders.
/// The stdin reader sends commands to all registered connections.
type ClientRegistry = Arc<Mutex<HashMap<u32, Sender<ControlCommand>>>>;

/// Monotonic handle counter for correlating commands with responses.
static HANDLE_COUNTER: AtomicU32 = AtomicU32::new(1);

fn next_handle() -> u32 {
    HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Parse a single stdin line into a [`ControlCommand`].
/// Returns `None` (with a logged warning) for unrecognised or malformed input.
fn parse_command(line: &str) -> Option<ControlCommand> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.splitn(2, char::is_whitespace);
    let verb = parts.next().unwrap();
    let rest = parts.next().unwrap_or("").trim_start();

    match verb {
        "sendsds" => parse_sendsds(rest),
        "help" => {
            println!("Available commands:");
            println!("  sendsds <source_ssi> <dest_ssi> <dest_is_group> <payload_hex>");
            println!("  help");
            None
        }
        other => {
            warn!("unknown command '{}'", other);
            None
        }
    }
}

/// Parse: `sendsds <source_ssi:u32> <dest_ssi:u32> <dest_is_group:bool> <payload_hex>`
fn parse_sendsds(args: &str) -> Option<ControlCommand> {
    let parts: Vec<&str> = args.splitn(4, char::is_whitespace).collect();
    if parts.len() < 4 {
        warn!("sendsds: expected 4 arguments: <source_ssi> <dest_ssi> <dest_is_group> <payload_hex>");
        return None;
    }

    let source_ssi = match parts[0].parse::<u32>() {
        Ok(v) => v,
        Err(_) => {
            warn!("sendsds: invalid source_ssi '{}'", parts[0]);
            return None;
        }
    };
    let dest_ssi = match parts[1].parse::<u32>() {
        Ok(v) => v,
        Err(_) => {
            warn!("sendsds: invalid dest_ssi '{}'", parts[1]);
            return None;
        }
    };
    let dest_is_group = match parts[2] {
        "true" | "1" => true,
        "false" | "0" => false,
        other => {
            warn!("sendsds: invalid dest_is_group '{}' (expected true/false/0/1)", other);
            return None;
        }
    };
    let payload_hex = parts[3].trim();
    let payload = match hex_decode(payload_hex) {
        Some(v) => v,
        None => {
            warn!("sendsds: invalid hex payload '{}'", payload_hex);
            return None;
        }
    };
    let len_bits = (payload.len() * 8) as u16;

    Some(ControlCommand::SendSds {
        handle: next_handle(),
        source_ssi,
        dest_ssi,
        dest_is_group,
        len_bits,
        payload,
    })
}

/// Decode a hex string (with or without 0x prefix, spaces allowed) into bytes.
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(&s);
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// Spawn the stdin reader thread.  Reads lines, parses commands, and
/// broadcasts each command to all registered connection threads.
fn spawn_stdin_reader(clients: ClientRegistry) {
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let reader = stdin.lock();
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!("stdin read error: {}", e);
                    break;
                }
            };
            if let Some(cmd) = parse_command(&line) {
                let registry = clients.lock().unwrap();
                if registry.is_empty() {
                    warn!("no connected base stations — command dropped");
                    continue;
                }
                let mut delivered = 0u32;
                for (id, tx) in registry.iter() {
                    if tx.send(cmd.clone()).is_ok() {
                        delivered += 1;
                    } else {
                        warn!("client {} channel closed", id);
                    }
                }
                info!("command dispatched to {} client(s)", delivered);
            }
        }
        info!("stdin closed");
    });
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

    let clients: ClientRegistry = Arc::new(Mutex::new(HashMap::new()));
    let next_client_id = Arc::new(AtomicU32::new(0));

    spawn_stdin_reader(Arc::clone(&clients));

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
        "Control receiver listening on {}{}{}",
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
                let clients_ref = Arc::clone(&clients);
                let client_id = next_client_id.fetch_add(1, Ordering::Relaxed);
                let (cmd_tx, cmd_rx) = unbounded::<ControlCommand>();

                // Register this client's command sender
                clients_ref.lock().unwrap().insert(client_id, cmd_tx);

                // Set a read timeout so the connection loop can interleave
                // reading WebSocket frames with sending outbound commands.
                let _ = tcp.set_read_timeout(Some(std::time::Duration::from_millis(100)));

                std::thread::spawn(move || {
                    if let Some(cfg) = tls_cfg {
                        let tls_conn = match rustls::ServerConnection::new(cfg) {
                            Ok(c) => c,
                            Err(e) => {
                                error!("[{}] TLS session init failed: {}", peer, e);
                                clients_ref.lock().unwrap().remove(&client_id);
                                return;
                            }
                        };
                        let tls_stream = rustls::StreamOwned::new(tls_conn, tcp);
                        handle_connection(tls_stream, &peer, auth.as_deref(), cmd_rx);
                    } else {
                        handle_connection(tcp, &peer, auth.as_deref(), cmd_rx);
                    }
                    // Unregister on disconnect
                    clients_ref.lock().unwrap().remove(&client_id);
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

fn handle_connection<S: Read + Write>(stream: S, peer: &str, auth_db: Option<&AuthDb>, cmd_rx: Receiver<ControlCommand>) {
    let callback = |req: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
        // Verify HTTP Basic Auth if enabled
        if !check_basic_auth(req, auth_db) {
            warn!("[{}] rejected: invalid or missing Basic Auth credentials", peer);
            let mut err = ErrorResponse::new(Some("401 Unauthorized".to_string()));
            err.headers_mut()
                .insert("WWW-Authenticate", "Basic realm=\"bluestation-control\"".parse().unwrap());
            return Err(err);
        }

        let proto = req
            .headers()
            .get("Sec-WebSocket-Protocol")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if proto.split(',').map(|s| s.trim()).any(|s| s == CONTROL_PROTOCOL_VERSION) {
            response
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", CONTROL_PROTOCOL_VERSION.parse().unwrap());
            Ok(response)
        } else {
            warn!(
                "[{}] rejected: expected subprotocol '{}', got '{}'",
                peer, CONTROL_PROTOCOL_VERSION, proto
            );
            Err(ErrorResponse::new(Some(format!(
                "unsupported subprotocol; expected {}",
                CONTROL_PROTOCOL_VERSION
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

    let codec = ControlCodecJson;
    info!("[{}] WebSocket connected", peer);

    loop {
        // --- send any pending outbound commands ---
        while let Ok(cmd) = cmd_rx.try_recv() {
            let payload = codec.encode_command(&cmd);
            info!("[{}] >> {:?}", peer, cmd);
            if let Err(e) = ws.send(Message::Binary(payload.into())) {
                error!("[{}] send error: {}", peer, e);
                return;
            }
        }

        // --- read one inbound frame (may time out due to read_timeout) ---
        match ws.read() {
            Ok(Message::Binary(data)) => match codec.decode_response(&data) {
                Ok(response) => {
                    info!("[{}] << {:?}", peer, response);
                }
                Err(e) => {
                    error!("[{}] deserialize error: {}", peer, e);
                }
            },
            Ok(Message::Text(text)) => {
                error!("[{}] unexpected text message ({} bytes), expected binary", peer, text.len());
            }
            Ok(Message::Ping(_)) => {
                if ws.flush().is_err() {
                    info!("[{}] failed to flush pong, disconnecting", peer);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                info!("[{}] client disconnected", peer);
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // Read timeout — loop back to check for outbound commands
                continue;
            }
            Err(tungstenite::Error::ConnectionClosed) => {
                info!("[{}] connection closed", peer);
                break;
            }
            Err(tungstenite::Error::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)) => {
                info!("[{}] connection reset", peer);
                break;
            }
            Err(e) => {
                error!("[{}] read error: {}", peer, e);
                break;
            }
        }
    }
}
