# BlueStation Wireshark Capture

`bluestation_type1.lua` decodes the custom UDP datagrams emitted by BlueStation's `[wireshark]` transport.

Example config:

```toml
[wireshark]
host = "127.0.0.1"
port = 42069
# pcap_file = "./bluestation-type1-debug.pcap"
```

Install in Wireshark by copying `bluestation_type1.lua` into your personal or global plugins directory, then restart Wireshark.

Notes:

- The dissector registers heuristically on UDP by checking for the `TBSW` magic header, so it can decode any configured destination port.
- Port `42069` is also registered directly as a convenience for the example config.
- If `pcap_file` is configured, BlueStation also writes synthetic Ethernet/IPv4/UDP packets carrying the same `TBSW` datagrams into a PCAP file so the capture can be opened directly in Wireshark later.
- BlueStation requires an interactive `yes` acknowledgement at startup before it will write a PCAP file.
- The script decodes the BlueStation wrapper, AACH `ACCESS-ASSIGN`, BSCH `MAC-SYNC` plus `D-MLE-SYNC`, BNCH `MAC-SYSINFO` plus `D-MLE-SYSINFO`, and the main SCH/STCH MAC headers.
- SCH/STCH TM-SDUs are further decoded through LLC and the MLE protocol discriminator into downlink MM and CMCE control PDUs when the MAC payload is not air-interface encrypted.
- Downlink MM coverage now includes location update accept/proceeding/reject, attach/detach group identity, and MM status with structured decoding of the typed optional fields already modeled in the Rust parser.
- Downlink CMCE coverage now includes alert, call proceeding, setup, connect, connect acknowledge, disconnect, release, status, and SDS data, including nested basic service information, party addressing, transmission grant, disconnect cause, pre-coded status, and SDS payload metadata.
- Uplink control PDUs and traffic-channel voice/data payloads are still exported and labeled, but not yet fully dissected.
