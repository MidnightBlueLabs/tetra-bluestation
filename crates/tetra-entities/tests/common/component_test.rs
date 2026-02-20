use tetra_config::{CfgCellInfo, CfgNetInfo, CfgPhyIo, PhyBackend, SharedConfig, StackConfig, StackMode, StackState};
use tetra_core::TdmaTime;
use tetra_core::freqs::FreqInfo;
use tetra_core::tetra_entities::TetraEntity;
use tetra_entities::{MessageRouter, TetraEntityTrait};
use tetra_saps::sapmsg::SapMsg;

// BS imports
use tetra_entities::cmce::cmce_bs::CmceBs;
use tetra_entities::cmce::cmce_ms::CmceMs;
use tetra_entities::llc::llc_bs_ms::Llc;
use tetra_entities::lmac::lmac_bs::LmacBs;
use tetra_entities::mle::mle_bs_ms::Mle;
use tetra_entities::mm::mm_bs::MmBs;
use tetra_entities::sndcp::sndcp_bs::Sndcp;
use tetra_entities::umac::umac_bs::UmacBs;

// MS imports
use tetra_entities::lmac::lmac_ms::LmacMs;
use tetra_entities::umac::umac_ms::UmacMs;

use super::sink::Sink;

/// Creates a default config for testing. It can still be modified as needed
/// before passing it to the ComponentTest constructor
pub fn default_test_config(stack_mode: StackMode) -> StackConfig {
    let net_info = CfgNetInfo { mcc: 204, mnc: 1337 };
    let freq_info = FreqInfo::from_components(4, 1521, 0, false, 4, None).unwrap();
    let mut cell_info = CfgCellInfo::default();
    cell_info.colour_code = 1;
    cell_info.location_area = 2;
    cell_info.main_carrier = freq_info.carrier;
    cell_info.freq_band = freq_info.band;
    cell_info.freq_offset_hz = freq_info.freq_offset_hz;
    cell_info.duplex_spacing_id = freq_info.duplex_spacing_id;
    cell_info.reverse_operation = freq_info.reverse_operation;
    let mut phy_io = CfgPhyIo::default();

    // These tests don't support a PHY component, so we set backend to None
    phy_io.backend = PhyBackend::None;

    // Put together components and return this proto config
    StackConfig {
        stack_mode,
        debug_log: None,
        phy_io,
        net: net_info,
        cell: cell_info,
        brew: None,
    }
}

/// Infrastructure for testing TETRA components
/// Quick setup of all components for end-to-end testing
/// Supports optional sinks for collecting messages for later inspection
pub struct ComponentTest {
    pub config: SharedConfig,
    pub router: MessageRouter,
    // components: Vec<TetraEntity>,
    pub sinks: Vec<TetraEntity>,
    start_dl_time: TdmaTime,
}

impl ComponentTest {
    pub fn new(config: StackConfig, start_dl_time: Option<TdmaTime>) -> Self {
        let shared_config = SharedConfig::from_parts(config, StackState::default());
        let config_clone = shared_config.clone();
        let mut mr = MessageRouter::new(config_clone);

        let start_dl_time = start_dl_time.unwrap_or_default();
        mr.set_dl_time(start_dl_time);

        Self {
            config: shared_config,
            router: mr,
            sinks: vec![],
            start_dl_time,
        }
    }

    pub fn get_shared_config(&self) -> SharedConfig {
        self.config.clone()
    }

    pub fn populate_entities(&mut self, components: Vec<TetraEntity>, sinks: Vec<TetraEntity>) {
        match self.config.config().stack_mode {
            StackMode::Bs => {
                self.create_components_bs(components);
            }
            StackMode::Ms => {
                self.create_components_ms(components);
            }
            _ => {
                panic!("Only BS stack mode is supported in ComponentTest");
            }
        }

        // Create sinks for debugging / message collection
        self.create_sinks(sinks);
    }

    fn create_components_bs(&mut self, components: Vec<TetraEntity>) {
        // Setup the stack with all requested components, performing set-up where needed
        for component in components.iter() {
            match component {
                TetraEntity::Lmac => {
                    let lmac = LmacBs::new(self.config.clone());
                    self.register_entity(lmac);
                }
                TetraEntity::Umac => {
                    let mut umac = UmacBs::new(self.config.clone());
                    // Prepare channel scheduler for next tick_start
                    umac.channel_scheduler.set_dl_time(self.start_dl_time.add_timeslots(-1));
                    self.router.register_entity(Box::new(umac));
                }
                TetraEntity::Llc => {
                    let llc = Llc::new(self.config.clone());
                    self.router.register_entity(Box::new(llc));
                }
                TetraEntity::Mle => {
                    let mle = Mle::new(self.config.clone());
                    self.router.register_entity(Box::new(mle));
                }
                TetraEntity::Mm => {
                    let mm = MmBs::new(self.config.clone());
                    self.router.register_entity(Box::new(mm));
                }
                TetraEntity::Sndcp => {
                    let sndcp = Sndcp::new(self.config.clone());
                    self.router.register_entity(Box::new(sndcp));
                }
                TetraEntity::Cmce => {
                    let cmce = CmceBs::new(self.config.clone());
                    self.router.register_entity(Box::new(cmce));
                }
                _ => {
                    panic!("Component not implemented: {:?}", component);
                }
            }
        }
    }

    fn create_components_ms(&mut self, components: Vec<TetraEntity>) {
        for component in components.iter() {
            match component {
                TetraEntity::Lmac => {
                    let lmac = LmacMs::new(self.config.clone());
                    self.router.register_entity(Box::new(lmac));
                }
                TetraEntity::Umac => {
                    let umac = UmacMs::new(self.config.clone());
                    self.router.register_entity(Box::new(umac));
                }
                TetraEntity::Llc => {
                    let llc = Llc::new(self.config.clone());
                    self.router.register_entity(Box::new(llc));
                }
                TetraEntity::Mle => {
                    let mle = Mle::new(self.config.clone());
                    self.router.register_entity(Box::new(mle));
                }
                TetraEntity::Cmce => {
                    let cmce = CmceMs::new(self.config.clone());
                    self.router.register_entity(Box::new(cmce));
                }
                _ => {
                    panic!("Component not implemented: {:?}", component);
                }
            }
        }
    }

    fn create_sinks(&mut self, sinks: Vec<TetraEntity>) {
        // Setup any sinks
        for sink in sinks.iter() {
            assert!(!self.sinks.contains(sink), "Sink already exists: {:?}", sink);
            assert!(
                self.router.get_entity(*sink).is_none(),
                "Sink already registered as entity: {:?}",
                sink
            );

            self.sinks.push(*sink);
            let sink = Sink::new(*sink);
            self.router.register_entity(Box::new(sink));
        }
    }

    pub fn register_entity<T: 'static + TetraEntityTrait>(&mut self, entity: T) {
        self.router.register_entity(Box::new(entity));
    }

    pub fn run_stack(&mut self, num_ticks: Option<usize>) {
        self.router.run_stack(num_ticks, None);
    }

    pub fn submit_message(&mut self, message: SapMsg) {
        self.router.submit_message(message);
    }

    pub fn deliver_all_messages(&mut self) {
        self.router.deliver_all_messages();
    }

    pub fn dump_sinks(&mut self) -> Vec<SapMsg> {
        let mut msgs = vec![];
        for sink in self.sinks.iter() {
            if let Some(component) = self.router.get_entity(*sink) {
                if let Some(sink) = component.as_any_mut().downcast_mut::<Sink>() {
                    let mut sink_msgs = sink.take_msgqueue();
                    msgs.append(&mut sink_msgs);
                }
            }
        }
        msgs
    }
}
