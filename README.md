[![Build Multi-Arch](https://github.com/web-rpi/tetra-bluestation/actions/workflows/build.yml/badge.svg)](https://github.com/web-rpi/tetra-bluestation/actions/workflows/build.yml)

```
░▀█▀░█▀▀░▀█▀░█▀▄░█▀█░░░░░█▀▄░█░░░█░█░█▀▀░█▀▀░▀█▀░█▀█░▀█▀░▀█▀░█▀█░█▀█
░░█░░█▀▀░░█░░█▀▄░█▀█░▄▄▄░█▀▄░█░░░█░█░█▀▀░▀▀█░░█░░█▀█░░█░░░█░░█░█░█░█
░░▀░░▀▀▀░░▀░░▀░▀░▀░▀░░░░░▀▀░░▀▀▀░▀▀▀░▀▀▀░▀▀▀░░▀░░▀░▀░░▀░░▀▀▀░▀▀▀░▀░▀
```

This is a FOSS TETRA stack aimed at providing an extensible basis for TETRA experimentation and research. At this point, the code is pre-alpha. The stack can serve an infrastructure signal, and a properly configured real-world MS is able to receive the emitted downlink signal, connect to it, and attach to talkgroups. All other functionality, including voice calls, is currently not implemented, although parsing code for most TETRA protocol messages is already present. 

## State of the project

### What works

- Base station configuration (freqs, network identifiers, etc) supplied as TOML file
- Demodulation and modulation of most burst types
- Will broadcast SYNC/SYSINFO and is identifiable as a proper TETRA network
- Phy, Lmac, Umac, Llc are mostly complete for BS use, although *there be bugs*, certain messages are not implemented / not implemented in a future-proof way
- Parsing/building code for mostly all core TETRA PDUs across all OSI layers are present and can be used as desired (currently unused types need sanity checking)
- Registration of an MS on the running BS stack and group attachment works

### Not functional / not implemented
- MS (Mobile Station, radio) and monitoring implementation not fully functional
- Mle and everything above is a skeleton implementation that has to be extended where needed
- Voice codec, call setup and calling is not implemented


## General TETRA design
We closely follow the TETRA standard and adopt a modular message passing framework. Messages can flow between the TETRA entities through well-defined SAPs, interfaces connecting the components. An outline of the entities and their connecting saps is shown below. 

![osi_layers](./figs/osi_layering.png)

For each SAP, message types are defined, called *primitives* or *prim* in short. These carry the actual encoded TETRA messages, called *PDUs*. 

A component will receive a prim on a specific SAP. It will unpack the prim, and depending on the prim, handle the contents of the PDU (air interface bits). If after parsing and processing, the PDU contains further data for next layers of the protocol (subpart of the PDU called SDU, basically the PDU for the next layer), the component creates a new prim (suitable for the SAP to which the message is now passed) and embeds the SDU.

## Basic usage example
**DISCLAIMER**: transmitting RF signals is only allowed under strict conditions and generally require a license. Make sure to comply with your local laws. 

- Connect a USRP, LimeSDR, LimeSDR Mini or sxceiver through USB
- Edit `example_bs_config.toml` and configure the properties of the network. The TOML format supports comments, making it easy to document your settings. Importantly, set the tx (downlink) and rx (uplink) frequencies, and make sure the `cell_info` values reflect these chosen frequencies. 
- Make a release build and start the base station stack. 
```
cargo build --release
./target/release/tetra-bluestation ./example_bs_config.toml
```
- You may want to run the applicaton as real-time for improved scheduling
```
chrt 99 ./target/release/tetra-bluestation ./example_bs_config.toml
```

## TETRALIB design

Firstly, the project constists of modules corresponding to all TETRA components as defined in the standard. These are referred to as *entities* and are:

- PHY
- LMAC
- UMAC
- LLC
- MLE
- MM
- CMCE
- SNDCP

### Example of the full-stack processing of a message
An annotated log of a full connection is found in `example_trace.txt`. The general process of establishing a connection and processing a registration is as follows.

The aforementioned TETRA entities are connected through a message queue, which accepts SAP primives. For example, PHY will receive data and create a TpUnitdataInd primitive, and send the raw Pi/4 DPSK demodulated bits to the LMAC over the TP-SAP. After processing and performing error correction, the LMAC will pass the data further up the OSI model, by creating a TmvUnitdataInd prim sent over the TMV-SAP. The UMAC will parse the message (it may contain, for example, a MAC-ACCESS) and if needed, perform defragmentation. Complete messages are sent further up to the LLC (for a unicast signalling message, the TMA-SAP), which maintains *links* bound to timeslots and addresses, featuring message acknowledgement, loss detection and retransmission. The LLC will send messages further up (TLA-SAP), either in a TL-DATA (acknowledged data service) or TL-UNITDATA (unacknowledged service) primitive, which now gets received by the MLE. The MLE may handle the message by itself, or pass it on to one of the top layer TETRA signalling components, the MM, CMCE or SNDCP. 

The top layer component may create one or more response messages, and push it to the queue. Say, the MM creates a LmmMleUnitdataReq message, which is passed down to the MLE, LLC and MAC. The MAC schedules the message and when the time is there, sends the message down to LMAC and PHY, after which the message is transmitted on the downlink. 

### Detailed component capabilities

#### PHY
The PHY layer has two backends, a file backend (for simulation/replay of traces) and the main SoapySDR backend, developed by Tatu Peltola. It handles rx/tx of the Pi/4 DPSK modulation. It detects incoming bursts, and when detected, passes them up to the LMAC. 

#### LMAC
The LMAC layer performs error detection and correction of incoming bursts, and transforms outgoing frames by adding the error correction data. It supports multiple burst types, all with different error correction characteristics. Correct incoming frames are passed up to the UMAC

#### UMAC
The UMAC layer handles unicast (individual signalling) and multicast (BS-to-MS broadcast messages) traffic. In BS mode, it maintains the UL schedule, registering when an MS is scheduled to send traffic. It also automatically handles requests for slot grants and sends acknowledgements for random access. Both are either sent as an empty MAC-RESOURCE, or combined with a MAC-RESOURCE that may already be scheduled for transmission (as requested by the LLC). It also performs Basic Link fragmentation (not currently implemented) and defragmentation (functional). Advanced Link is not supported. 

#### LLC
The LLC maintains state on unicast links, and will schedule an acknowledgement whenever a message for the acknowledged data service is received (BL-DATA, BL-ADATA). Received messages are optionally checked for integrity if an FCS is present, and are passed up to the MLE. If the MLE delivers a response, the LLC will detect this, and combine an outstanding acknowledgement with the resource, sending a BL-ADATA instead of a BL-ACK and a separate BL-DATA. The LLC also maintains whether sent messages are acknowledged by the MS, however, at this point, the retransmission logic when no ack is received is still missing. 

#### MLE
The MLE just hands packets between the LLC and one of the upper layer components (MM, CMCE, SNDCP). Other features are mostly for MS use, which is out of scope for this project.

#### MM
The MM is mostly a skeleton implementation, however, two functions are implemented. 

- The U-LOCATION UPDATE DEMAND is handled by sending a D-LOCATION UPDATE ACCEPT, allowing for registration of an MS. 
- The U-ATTACH/DETACH GROUP IDENTITY is handled by sending a D-ATTACH/DETACH GROUP IDENTITY ACKNOWLEDGEMENT, allowing an MS to attach to a group. 

This is sufficient for a full association/registration with an MS. 

#### SNDCP
Only a skeleton implementation, printing a received message type, is currently present. However, parsing logic for most PDUs is available to make extension relatively straightforward. 

#### CMCE
Only a skeleton implementation, printing a received message type, is currently present. However, parsing logic for most PDUs is available to make extension relatively straightforward. 

## Acknowledgements

- Thanks to Harald Welte and the osmocom crew for their amazing initial work on osmocom-tetra, without which this project would not have existed. 
- Many thanks to Tatu Peltola, who graciously augmented rust-soapysdr with the required timestamping functionality to facilitate robust rx/tx, and also provided a rust-native Viterbi encoder/decoder class used in the LMAC. 
- Thanks to Stichting NLnet, who agreed on allocating a part of the [RETETRA3 project](https://nlnet.nl/project/RETETRA3/) grant to the implementation of FOSS software for TETRA. 
