use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use tetra_entities::tnmm_net::codec::*;
use tetra_entities::tnmm_net::test_pdu::*;

const BIND_ADDR: &str = "127.0.0.1:8443";

/// TETRA Test  Centre implementation
fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(BIND_ADDR)?;
    println!("QUIC Testing Server listening on {}", BIND_ADDR);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New client connection: {:?}", stream.peer_addr());
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Handle a single client connection using the stack's message protocol.
/// Thread remains active until the client disconnects or an error occurs
fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    // Set timeouts
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let peer_addr = stream.peer_addr()?;

    // Keep listening for messages for this client, until something fails
    loop {
        // Read message length first (4 bytes, big endian)
        let mut msg_len_buf = [0u8; 4];
        let msg_len = match stream.read_exact(&mut msg_len_buf) {
            Ok(()) => {
                // Receive 4-byte len field
                let msg_len = u32::from_be_bytes(msg_len_buf) as usize;
                if msg_len == 0 {
                    tracing::warn!("Client {:?} sent empty message, closing", peer_addr);
                    break;
                }
                if msg_len > 1024 * 1024 {
                    // 1MB max message size
                    tracing::warn!("Message too large from {:?}: {} bytes", peer_addr, msg_len);
                    break;
                }
                msg_len
            }
            Err(e) => {
                match e.kind() {
                    std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset => {
                        tracing::debug!("Client {:?} disconnected cleanly", peer_addr);
                    }
                    std::io::ErrorKind::TimedOut => {
                        tracing::warn!("Client {:?} connection timed out", peer_addr);
                    }
                    _ => {
                        tracing::error!("Client {:?} error: {}", peer_addr, e);
                    }
                }
                break;
            }
        };

        // Read the actual message payload
        let mut msg_buf = vec![0u8; msg_len];
        stream.read_exact(&mut msg_buf)?;
        tracing::info!("<- {} bytes from {:?}", msg_buf.len(), peer_addr);

        // Deserialize using TestPduCodec
        let codec = TnmmTestCodec::default();
        let Ok(request_pdu) = codec.decode(&msg_buf) else {
            tracing::warn!("Failed to decode PDU from {:?}", peer_addr);
            break;
        };

        // Process pdu, check ServiceID and version
        tracing::debug!("Received PDU {:?}", request_pdu);
        if request_pdu.service_id != TEST_SERVICE_ID {
            tracing::warn!(
                "Service ID mismatch: expected 0x{:08X}, got 0x{:08X}",
                TEST_SERVICE_ID,
                request_pdu.service_id
            );
            continue;
        }
        if request_pdu.version != TEST_PDU_VERSION {
            tracing::warn!(
                "Protocol version mismatch: expected {}, got {}",
                TEST_PDU_VERSION,
                request_pdu.version
            );
            continue;
        }

        let response_pdu = match request_pdu.payload {
            Some(TestPdu::HeartbeatTick(tick)) => pack_test_pdu(TestPdu::HeartbeatTock(process_heartbeat_tick(tick))),
            Some(TestPdu::TestRequest(request)) => pack_test_pdu(TestPdu::TestResponse(process_test_request(request))),
            _ => {
                println!("Unexpected PDU type received");
                continue;
            }
        };
        tracing::info!("-> {:?}", response_pdu);

        // Send PDU response back to client using codec
        if let Err(e) = send_pdu_response(&mut stream, &codec, &response_pdu) {
            tracing::error!("Failed to send response: {}", e);
            break;
        }
    }

    println!("Client {:?} session ended", peer_addr);
    Ok(())
}

/// Process a HeartbeatTick and generate HeartbeatTock response
fn process_heartbeat_tick(tick: HeartbeatTick) -> HeartbeatTock {
    println!("Processing HeartbeatTick with handle: {}", tick.handle);

    HeartbeatTock { handle: tick.handle }
}

/// Process a TestRequest and generate TestResponse
fn process_test_request(request: TestRequest) -> TestResponse {
    println!("Processing TestRequest - handle: {}, ssi: {}", request.handle, request.ssi);

    // Generate bogus test data
    let test_data = 0x1234ABCD;

    TestResponse {
        handle: request.handle,
        ssi: request.ssi,
        data: test_data,
    }
}

/// Send a PDU response using TCP framing (length + payload)
fn send_pdu_response(stream: &mut TcpStream, codec: &TnmmTestCodec, pdu: &TnmmTestPduEnvelope) -> std::io::Result<()> {
    let response_bytes = codec
        .encode(pdu)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let length = response_bytes.len() as u32;

    // Send length first (4 bytes, big endian)
    stream.write_all(&length.to_be_bytes())?;

    // Send the encoded payload
    stream.write_all(&response_bytes)?;
    stream.flush()?;

    println!("Sent PDU response ({} bytes): {:?}", response_bytes.len(), pdu);
    Ok(())
}
