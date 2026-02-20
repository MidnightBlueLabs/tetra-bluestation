use quinn::{Endpoint, ServerConfig};
/// Simple QUIC-based testing server
/// Make sure it is running before running the QUIC tests
use std::sync::Arc;
use std::time::Duration;
use tetra_entities::tnmm_net::codec::TnmmTestCodec;
use tetra_entities::tnmm_net::test_pdu::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    tracing_subscriber::fmt::init();

    println!("QUIC Testing Server starting...");

    // Generate self-signed certificate (for testing only!)
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())?;

    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)?;

    // Allow TLS 1.3 only for QUIC
    server_crypto.alpn_protocols = vec![b"hq-29".to_vec()];

    let mut server_config = ServerConfig::with_crypto(Arc::new(quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?));

    // Configure transport
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.max_concurrent_bidi_streams(100u32.into());
    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));

    server_config.transport_config(Arc::new(transport_config));

    let endpoint = Endpoint::server(server_config, "[::]:4433".parse()?)?;
    println!("QUIC Testing Server listening on [::]:4433");

    while let Some(connecting) = endpoint.accept().await {
        tokio::spawn(async move {
            match connecting.await {
                Ok(connection) => {
                    println!("Client connected: {}", connection.remote_address());
                    if let Err(e) = handle_connection(connection).await {
                        eprintln!("Connection error: {}", e);
                    }
                }
                Err(e) => eprintln!("Connection failed: {}", e),
            }
        });
    }

    Ok(())
}

async fn handle_connection(connection: quinn::Connection) -> Result<(), Box<dyn std::error::Error>> {
    let codec = TnmmTestCodec::default();

    // Accept bidirectional stream for signalling
    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
        println!("Accepted bidirectional stream");

        // Read length-prefixed messages
        loop {
            // Read 4-byte length header
            let mut len_buf = [0u8; 4];
            match recv.read_exact(&mut len_buf).await {
                Ok(()) => {
                    let msg_len = u32::from_be_bytes(len_buf) as usize;

                    if msg_len == 0 || msg_len > 1024 * 1024 {
                        println!("Invalid message length: {}", msg_len);
                        break;
                    }

                    // Read payload
                    let mut payload = vec![0u8; msg_len];
                    recv.read_exact(&mut payload).await?;

                    println!("Received {} bytes", payload.len());

                    // Decode PDU
                    let request_pdu = match codec.decode(&payload) {
                        Ok(pdu) => pdu,
                        Err(e) => {
                            eprintln!("Failed to decode PDU: {}", e);
                            continue;
                        }
                    };

                    println!("Decoded PDU: {:?}", request_pdu);

                    // Check service ID and version
                    if request_pdu.service_id != TEST_SERVICE_ID {
                        eprintln!(
                            "Service ID mismatch: expected 0x{:08X}, got 0x{:08X}",
                            TEST_SERVICE_ID, request_pdu.service_id
                        );
                        continue;
                    }

                    if request_pdu.version != TEST_PDU_VERSION {
                        eprintln!(
                            "Protocol version mismatch: expected {}, got {}",
                            TEST_PDU_VERSION, request_pdu.version
                        );
                        continue;
                    }

                    // Process request and generate response
                    let response_pdu = match request_pdu.payload {
                        Some(TestPdu::HeartbeatTick(tick)) => {
                            println!("Processing heartbeat tick: {}", tick.handle);
                            pack_test_pdu(TestPdu::HeartbeatTock(HeartbeatTock { handle: tick.handle }))
                        }
                        Some(TestPdu::TestRequest(request)) => {
                            println!("Processing TestRequest - handle: {}, ssi: {}", request.handle, request.ssi);

                            // Generate test data
                            let test_data = 0x1234ABCD;

                            pack_test_pdu(TestPdu::TestResponse(TestResponse {
                                handle: request.handle,
                                ssi: request.ssi,
                                data: test_data,
                            }))
                        }
                        _ => {
                            println!("Unexpected PDU type");
                            continue;
                        }
                    };

                    // Encode response
                    let response_bytes = codec.encode(&response_pdu)?;

                    // Send length-prefixed response
                    let len = (response_bytes.len() as u32).to_be_bytes();
                    send.write_all(&len).await?;
                    send.write_all(&response_bytes).await?;

                    println!("Sent response: {} bytes", response_bytes.len());
                }
                Err(_) => {
                    println!("Stream closed by client");
                    break;
                }
            }
        }
    }

    Ok(())
}
