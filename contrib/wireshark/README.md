# BlueStation Wireshark Capture

`bluestation_type1.lua` decodes the custom UDP datagrams emitted by BlueStation's `[wireshark]` transport.

Example config:

```toml
[wireshark]
host = "127.0.0.1"
port = 42069
```

Install in Wireshark by copying `bluestation_type1.lua` into your personal or global plugins directory, then restart Wireshark.

Notes:

- The dissector registers heuristically on UDP by checking for the `TBSW` magic header, so it can decode any configured destination port.
- Port `42069` is also registered directly as a convenience for the example config.
- The script decodes the BlueStation wrapper, AACH `ACCESS-ASSIGN`, BSCH `MAC-SYNC` plus `D-MLE-SYNC`, BNCH `MAC-SYSINFO` plus `D-MLE-SYSINFO`, and the main SCH/STCH MAC headers.
- Traffic-channel type-1 payloads are exported and labeled, but not further dissected.
