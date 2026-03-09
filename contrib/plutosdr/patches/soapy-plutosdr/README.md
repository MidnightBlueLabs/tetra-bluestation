# SoapyPlutoSDR Patches

Diese Patches werden auf [pgreenland's SoapyPlutoSDR](https://github.com/pgreenland/SoapyPlutoSDR) (Branch: `sdr_gadget_timestamping_with_iio_support`) angewendet.

## Patches

### 1. Tick-Base Overflow Prevention (`01-tick-base-overflow.patch`)

**Problem:** Der FPGA-Sample-Counter ist ein frei laufender uint64, der bei einem grossen Wert startet. Wenn `SoapySDR::ticksToTimeNs()` diesen mit 1e9 multipliziert, ueberlaeuft int64.

**Loesung:** Den ersten empfangenen Timestamp als "Tick-Base" von allen folgenden Timestamps subtrahieren. Auf der TX-Seite die Tick-Base wieder addieren bevor zum FPGA geschrieben wird.

**Betroffene Dateien:**
- `SoapyPlutoSDR.hpp` - `shared_tick_base` und `shared_tick_base_set` Members
- `PlutoSDR_Settings.cpp` - Tick-Base im Konstruktor initialisieren
- `PlutoSDR_Streaming.cpp` - Tick-Base in RX/TX und IIO-Streamern verdrahten
- `PlutoSDR_RXStreamerUSBGadget.hpp/cpp` - Tick-Base-Subtraktion im recv
- `PlutoSDR_TXStreamerUSBGadget.hpp/cpp` - Tick-Base-Addition im send
- `PlutoSDR_RXStreamerIPGadget.hpp/cpp` - Gleiches Muster
- `PlutoSDR_TXStreamerIPGadget.hpp/cpp` - Gleiches Muster

### 2. USB Gadget mit Network IIO Context (`02-usb-gadget-network-iio.patch`)

**Problem:** `open_sdr_usb_gadget()` funktioniert nur wenn der IIO-Context USB ist (`usb:X.Y`). Bei Verbindung ueber `ip:192.168.2.1` kann der USB-Gadget nicht geoeffnet werden.

**Loesung:** `handle_direct_args()` modifiziert, um USB-Gadget-Verbindung unabhaengig vom IIO-Context-Typ zu versuchen. `open_sdr_usb_gadget()` modifiziert, um PlutoSDR per USB Vendor/Product ID (0456:b673) zu finden wenn die URI keine USB Bus/Device-Nummern enthaelt. Fallback auf IP-Gadget bei Netzwerk-Verbindung.

**Betroffene Dateien:**
- `PlutoSDR_Settings.cpp` - `handle_direct_args()` und `open_sdr_usb_gadget()`

## Anwenden

```bash
git clone https://github.com/pgreenland/SoapyPlutoSDR.git
cd SoapyPlutoSDR
git checkout sdr_gadget_timestamping_with_iio_support

# Alle Patches auf einmal anwenden
git apply ../bluestation-plutosdr/patches/soapy-plutosdr/full-plutosdr-patches.patch

# Bauen
mkdir build && cd build
cmake ..
make -j$(nproc)
sudo make install
```

## Verifizieren

```bash
SoapySDRUtil --probe="driver=plutosdr,uri=ip:192.168.2.1"
```

Die installierte Library sollte hier liegen:
```bash
ls -la /usr/local/lib/SoapySDR/modules0.8/libPlutoSDRSupport.so
```
