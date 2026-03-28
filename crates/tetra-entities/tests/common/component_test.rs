use tetra_config::bluestation::{SharedConfig, StackConfig, StackMode};
use tetra_core::TdmaTime;
use tetra_core::tetra_entities::TetraEntity;
use tetra_entities::{MessageRouter, TetraEntityTrait};
use tetra_saps::sapmsg::SapMsg;

// BS imports
use tetra_entities::cmce::cmce_bs::CmceBs;
use tetra_entities::cmce::cmce_ms::CmceMs;
use tetra_entities::llc::llc_bs_ms::Llc;
use tetra_entities::lmac::lmac_bs::LmacBs;
use tetra_entities::mle::mle_bs::MleBs;
use tetra_entities::mm::mm_bs::MmBs;
use tetra_entities::sndcp::sndcp_bs::Sndcp;
use tetra_entities::umac::umac_bs::UmacBs;

// MS imports
use tetra_entities::lmac::lmac_ms::LmacMs;
use tetra_entities::umac::umac_ms::UmacMs;

use crate::common::default_stack;

use super::sink::Sink;

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
    /// Get a default test config for the given stack mode, suitable for testing
    /// stack components. The config can be further modified if needed before creating the ComponentTest instance.
    pub fn get_default_test_config(stack_mode: StackMode) -> StackConfig {
        match stack_mode {
            StackMode::Bs => default_stack::default_test_config_bs(),
            StackMode::Ms => default_stack::default_test_config_ms(),
            _ => panic!("Unsupported stack mode"),
        }
    }

    /// Create a new ComponentTest instance with the given config and optional start downlink time.
    pub fn from_config(config: StackConfig, start_dl_time: Option<TdmaTime>) -> Self {
        let shared_config = SharedConfig::from_parts(config, None);
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

    /// Create a new ComponentTest instance with a default config for the given stack mode.
    /// Optionally specify a start downlink time.
    pub fn new(stack_mode: StackMode, start_dl_time: Option<TdmaTime>) -> Self {
        let config = Self::get_default_test_config(stack_mode);
        Self::from_config(config, start_dl_time)
    }

    /// Get a cloned copy of the ComponentTest's shared config
    pub fn get_shared_config(&self) -> SharedConfig {
        self.config.clone()
    }

    /// Automatically add a set of components and sinks to the stack. Depending on the stack mode,
    /// the components will be created with the appropriate BS or MS implementations.
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
                    let mle = MleBs::new(self.config.clone());
                    self.router.register_entity(Box::new(mle));
                }
                TetraEntity::Mm => {
                    let mm = MmBs::new(self.config.clone(), None);
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
                    let mle = MleBs::new(self.config.clone());
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
