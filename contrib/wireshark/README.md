# BlueStation Wireshark Capture

`bluestation_type1.lua` decodes the custom UDP datagrams emitted by BlueStation's `[wireshark]` transport.

Example config:

```toml
[wireshark]
host = "127.0.0.1"
port = 42069
# pcap_file = "./bluestation-type1-debug.pcap"
# suppress_d_mle_sync = true
# suppress_d_mle_sysinfo = true
```

Install in Wireshark by copying `bluestation_type1.lua` into your personal or global plugins directory, then restart Wireshark.

Useful display filters:

```wireshark
udp.port == 42069
```

```wireshark
bluestation.type1.direction == 1
```

```wireshark
bluestation.type1.direction == 0
```

```wireshark
bluestation.type1.direction == 1 && bluestation.type1.crc_pass == 0
```

```wireshark
bluestation.type1.direction == 1 && bluestation.type1.logical_channel == 7
```

```wireshark
bluestation.type1.direction == 1 && bluestation.type1.logical_channel == 5
```

```wireshark
bluestation.type1.logical_channel != 2 && bluestation.type1.logical_channel != 3
```

Field values:

- `bluestation.type1.direction`: `0 = Downlink`, `1 = Uplink`
- `bluestation.type1.crc_pass`: `0 = false`, `1 = true`
- `bluestation.type1.logical_channel`: `2 = BSCH`, `3 = BNCH`, `5 = SCH/F`, `7 = SCH/HU`, `8 = TCH/S`

Notes:

- The dissector registers heuristically on UDP by checking for the `TBSW` magic header, so it can decode any configured destination port.
- Port `42069` is also registered directly as a convenience for the example config.
- If `pcap_file` is configured, BlueStation also writes synthetic Ethernet/IPv4/UDP packets carrying the same `TBSW` datagrams into a PCAP file so the capture can be opened directly in Wireshark later.
- BlueStation requires an interactive `yes` acknowledgement at startup before it will write a PCAP file.
- `suppress_d_mle_sync` drops BSCH `D-MLE-SYNC` exports at the source.
- `suppress_d_mle_sysinfo` drops BNCH `D-MLE-SYSINFO` exports at the source.
- The script decodes the BlueStation wrapper, AACH `ACCESS-ASSIGN`, BSCH `MAC-SYNC` plus `D-MLE-SYNC`, BNCH `MAC-SYSINFO` plus `D-MLE-SYSINFO`, and the main SCH/STCH MAC headers.
- The script also decodes downlink MLE `D-NWRK-BROADCAST` payloads carried through LLC/TL-SDU.
- SCH/STCH TM-SDUs are further decoded through LLC and the MLE protocol discriminator into downlink MM and CMCE control PDUs when the MAC payload is not air-interface encrypted.
- Downlink MM coverage now includes location update accept/proceeding/reject, attach/detach group identity, and MM status with structured decoding of the typed optional fields already modeled in the Rust parser.
- Downlink CMCE coverage now includes alert, call proceeding, setup, connect, connect acknowledge, disconnect, release, status, and SDS data, including nested basic service information, party addressing, transmission grant, disconnect cause, pre-coded status, and SDS payload metadata.
- Uplink control PDUs and traffic-channel voice/data payloads are still exported and labeled, but not yet fully dissected.
