# TETRA Bluestation mit ADALM-Pluto SDR

Eine vollstaendige Anleitung zum Betrieb einer TETRA BTS (Base Transceiver Station) mit [tetra-bluestation](https://github.com/MidnightBlueLabs/tetra-bluestation) auf einem [ADALM-Pluto SDR](https://www.analog.com/en/resources/evaluation-hardware-and-software/evaluation-boards-kits/adalm-pluto.html).

> **Hinweis:** Dieses Setup ist fuer den Amateurfunkbetrieb auf dem 70cm-Band (430-440 MHz) mit entsprechender Lizenz gedacht. Senden auf TETRA-Frequenzen ohne Genehmigung ist illegal.

## Uebersicht

```
┌──────────────┐     USB      ┌──────────────────┐
│  ADALM-Pluto │◄────────────►│   Linux PC       │
│  (Custom FW) │  IIO Control │                  │
│              │  + USB Gadget │  tetra-bluestation│
│  FPGA with   │   Streaming  │  (Rust)          │
│  Timestamps  │              │       │          │
└──────────────┘              │  rust-soapysdr   │
                              │       │          │
                              │  SoapyPlutoSDR   │
                              │  (modified C++)  │
                              └──────────────────┘
```

Der ADALM-Pluto laeuft mit einer Custom-Firmware mit FPGA-basiertem Sample-Timestamping. Der Host-PC laeuft tetra-bluestation, das ueber einen modifizierten SoapyPlutoSDR-Treiber kommuniziert.

## Getestete Konfiguration

| Komponente | Version |
|-----------|---------|
| OS | Debian 12/13 (x86_64) |
| Rust | 1.94+ |
| ADALM-Pluto FW | pgreenland v0.38_with_timestamping |
| SoapySDR | 0.8.x |
| libiio | 0.24+ |
| libad9361 | 0.3+ |
| libusb | 1.0.x |

## Hardware-Voraussetzungen

- **ADALM-Pluto SDR** (Rev.B oder Rev.C)
- **Linux PC** mit USB-Port (min. 4 GB RAM fuer Kompilierung)
- **Antenne** oder Dummy-Load am TX-Port
- Optional: Externer 10/20 MHz Referenz-Oszillator (GPS-diszipliniert) fuer bessere Frequenzgenauigkeit

## Software-Abhaengigkeiten

```bash
sudo apt install build-essential cmake git cargo rustc \
    libclang-dev libsoapysdr-dev libiio-dev libad9361-dev \
    libusb-1.0-0-dev soapysdr-tools sshpass libiio-utils
```

## Repository-Struktur

```
├── README.md                     # Diese Anleitung
├── patches/
│   ├── soapy-plutosdr/           # Patches fuer SoapyPlutoSDR-Treiber
│   │   └── 01-tick-base-overflow.patch
│   │   └── 02-usb-gadget-network-iio.patch
│   └── tetra-bluestation/        # Patches fuer tetra-bluestation
│       └── 01-plutosdr-config.patch
│       └── 02-usb-direct-device-arg.patch
│       └── 03-tx-timestamp-every-write.patch
│       └── 04-disable-get-hardware-time.patch
│       └── 05-plutosdr-defaults.patch
├── config/
│   └── config.toml.example       # Beispiel-Konfiguration
├── tools/
│   ├── measure_ppm.py            # PPM-Kalibrierung (2-Schritt)
│   ├── measure_pluto_tx.py       # PlutoSDR TX-Offset messen
│   └── measure_pluto_rx.py       # PlutoSDR RX-Offset messen (LeoBodnar)
├── firmware/
│   └── README.md                 # Firmware-Flash-Anleitung
└── scripts/
    └── restart_dashboard.sh      # Dashboard-Neustart
```

## Schnellstart

1. **Firmware flashen** - siehe `firmware/README.md`
2. **SoapyPlutoSDR patchen & bauen** - siehe `patches/soapy-plutosdr/README.md`
3. **tetra-bluestation patchen & bauen** - siehe `patches/tetra-bluestation/README.md`
4. **Konfiguration anpassen** - siehe `config/config.toml.example`
5. **Starten** - `./target/release/tetra-bluestation config.toml`

## Frequenzberechnung

```
DL_freq = Band × 100 MHz + Carrier × 25 kHz + Freq_Offset
        = 4 × 100 MHz + 1533 × 25 kHz + 0
        = 438.325 MHz

UL_freq = DL_freq - Duplex_Spacing
        = 438.325 MHz - 7.6 MHz
        = 430.725 MHz
```

## Bekannte Einschraenkungen

### Frequenzgenauigkeit

Der interne 40 MHz TCXO des ADALM-Pluto hat ±25 ppm Genauigkeit. TETRA erfordert ±0.25 ppm. Bei 438 MHz ergibt das bis zu ~11 kHz Frequenzfehler.

**Loesungen:**
- `ppm_err` in config.toml nach Messung anpassen
- Pluto XO-Korrektur kalibrieren: `ssh root@192.168.2.1 "echo 40000000 > /sys/bus/iio/devices/iio:device0/xo_correction"`
- Externen GPS-disziplinierten Referenz-Oszillator verwenden (empfohlen)
- PPM-Messtools verwenden (siehe `tools/`)

### Normale Startup-Warnungen

Diese Warnungen beim Start sind normal:
- `Lost -1152 samples` - Erster Buffer-Alignment
- `Too late to produce TX block 0, skipping 5 TX blocks` - Initiale Sync-Verzoegerung
- `Failed to set RX/TX thread priority` - Kein Root, kein Einfluss auf Funktionalitaet

## Troubleshooting

| Problem | Loesung |
|---------|---------|
| "Bad URI" / "no device context found" | `ping 192.168.2.1`, `iio_info -s` |
| "libusb failed to open device (-3)" | udev-Regel erstellen (siehe unten) |
| "USB direct mode failed" | `ssh root@192.168.2.1 "ps \| grep sdr"`, `lsusb \| grep Analog` |
| Terminal findet BTS nicht | TX-Frequenz pruefen, TX-Gain erhoehen, Duplex-Spacing pruefen |
| Timestamp overflow / crash | SoapyPlutoSDR-Patch korrekt installiert? |

### udev-Regel fuer PlutoSDR

```bash
sudo tee /etc/udev/rules.d/90-plutosdr.rules << 'EOF'
SUBSYSTEM=="usb", ATTR{idVendor}=="0456", ATTR{idProduct}=="b673", MODE="0666"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Referenzen

- [tetra-bluestation](https://github.com/MidnightBlueLabs/tetra-bluestation) - TETRA Base Station Stack (Rust)
- [tetra-bluestation Docs Wiki](https://github.com/MidnightBlueLabs/tetra-bluestation-docs/wiki)
- [pgreenland/plutosdr-fw](https://github.com/pgreenland/plutosdr-fw) - Custom PlutoSDR Firmware mit FPGA Timestamps
- [pgreenland/SoapyPlutoSDR](https://github.com/pgreenland/SoapyPlutoSDR) - SoapySDR Treiber mit Timestamp-Support
- [ETSI TS 100 392-15](https://www.etsi.org/deliver/etsi_ts/100300_100399/10039215/) - TETRA Frequenzbaender und Duplex-Spacing

## Lizenz

MIT - Diese Anleitung und die zugehoerigen Patches werden as-is fuer Bildungs- und Amateurfunkzwecke bereitgestellt.
