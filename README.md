```
░▀█▀░█▀▀░▀█▀░█▀▄░█▀█░░░░░█▀▄░█░░░█░█░█▀▀░█▀▀░▀█▀░█▀█░▀█▀░▀█▀░█▀█░█▀█
░░█░░█▀▀░░█░░█▀▄░█▀█░▄▄▄░█▀▄░█░░░█░█░█▀▀░▀▀█░░█░░█▀█░░█░░░█░░█░█░█░█
░░▀░░▀▀▀░░▀░░▀░▀░▀░▀░░░░░▀▀░░▀▀▀░▀▀▀░▀▀▀░▀▀▀░░▀░░▀░▀░░▀░░▀▀▀░▀▀▀░▀░▀
```

This is a FOSS TETRA stack aimed at providing an extensible basis for TETRA experimentation and research. At this point, it's alpha code. The stack serves a downlink base station signal, and a properly configured MS is able to receive the emitted downlink signal, connect to it, and attach to talkgroups. Voice calls are partially supported. Connectivity through Brew with the larger BrandMeister network is also optionally available. Lots of other functionality is currently not implemented, although parsing code for most TETRA protocol messages is already present. 

## Documentation

Project documentation for tetra-bluestation is maintained in a separate repository, as a wiki.

https://github.com/MidnightBlueLabs/tetra-bluestation-docs/wiki

The documentation repository contains:
- Hardware and SDR considerations 
- Configuration file reference and examples  
- Build and runtime instructions   
- Practical notes 

Contributions to the documentation follow the same pull-request-based workflow as the main codebase, see the appropriate "Contributions" chapter.

## Acknowledgements

- Thanks to Harald Welte and the osmocom crew for their amazing initial work on osmocom-tetra, without which this project would not have existed. 
- Many thanks to Tatu Peltola, who graciously augmented rust-soapysdr with the required timestamping functionality to facilitate robust rx/tx, and also provided a rust-native Viterbi encoder/decoder class used in the LMAC.
- Many thanks to the awesome contributers helping to make BlueStation as stable, fancy and feature-rich as can be. 
- Thanks to Stichting NLnet, who agreed on allocating a part of the [RETETRA3 project](https://nlnet.nl/project/RETETRA3/) grant to the implementation of FOSS software for TETRA. 
