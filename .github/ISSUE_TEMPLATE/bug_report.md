---
name: Bug report
about: Create a report to help us improve
title: ''
labels: ''
assignees: ''

---

## Bug description

**Describe the bug**  
*A clear and concise description of what the bug is.*

## Steps to reproduce

### Hardware

*List of hardware used*

- *SDR Model*
- *Radio model*
- *Hardware running `tetra-bluestation*

### Software

*Steps to reproduce the bug*

- *Build and launch `tetra-bluestation` from the <Branch name> branch*
- *Operation that causes the bug*
- ...

## Expected behavior

*A clear and concise description of what you expected to happen. (optional)*

## Command output

*The  **exact** content in you console when the bug happens; for example : *

```zsh
 9:51:18 ~/Downloads/tetra-bluestation
$ ./target/release/tetra-bluestation ./example_config/config_bel.toml | grep -v "phy\|common\|lmac"

DEBUG src/entities/umac/subcomp/defrag.rs:101: Defrag buffer 0 first: ssi: 2065022, t_first: 1, t_last: 1, num_frags: 1: 00011001011100111000000011111100001000010000000000000000^
DEBUG src/entities/umac/subcomp/bs_channel_scheduler.rs:412: dl_integrate_sched_elems_for_timeslot: Creating new resource for addr SSI:2065022 with ack
DEBUG src/entities/umac/subcomp/bs_channel_scheduler.rs:393: dl_integrate_sched_elems_for_timeslot: Integrating grant BasicSlotgrant { capacity_allocation: Grant1Slot, granting_delay: CapAllocAtNextOpportunity } into resource for addr SSI:2065022
DEBUG src/entities/umac/subcomp/bs_channel_scheduler.rs:499: finalize_ts_for_tick: inserting MacResource { fill_bits: true, pos_of_grant: 0, encryption_mode: 0, random_access_flag: true, length_ind: 7, addr: Some(TetraAddress { encrypted: false, ssi_type: Ssi, ssi: 2065022 }), event_label: None, usage_marker: None, power_control_element: None, slot_granting_element: Some(BasicSlotgrant { capacity_allocation: Grant1Slot, granting_delay: CapAllocAtNextOpportunity }), chan_alloc_element: None } sdu ^

thread 'main' (2165443) panicked at src/entities/umac/umac_bs.rs:844:9:
not implemented: rx_ul_mac_end
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
DEBUG src/entities/umac/umac_bs.rs:1126: rx_prim: SapMsg { sap: TmvSap, src: Lmac, dest: Umac, dltime:     0/05/15/3, msg: TmvUnitdataInd(TmvUnitdataInd { pdu: BitBuffer { <0 ^0 >268 ^0110001110000000000010010000000000000000000000000100010000000000000000000000000110010000000000000000000000001000001000000111111000001001111110000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 }, logical_channel: SchF, crc_pass: true, scrambling_code: 864282631 }) }
DEBUG src/entities/umac/umac_bs.rs:834: <- MacEndUl { fill_bits: false, length_ind: Some(14), reservation_req: None }

```

## Screenshots

*If applicable, add screenshots to help explain your problem.*

## `Config.toml`

*If applicable, add the content of your `config.toml` file.*

```toml
config_version = "0.4"
stack_mode = "Bs"

# PHY layer i/o configuration

[phy_io]
backend = "SoapySdr"

...
```
