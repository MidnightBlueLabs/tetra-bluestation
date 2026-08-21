use std::collections::{HashMap, HashSet};

use tetra_pdus::mm::{enums::energy_saving_mode::EnergySavingMode, fields::class_of_ms::ClassOfMs};

#[derive(Debug, Clone)]
pub struct Subscriber {
    pub issi: u32,
    pub attached_groups: HashSet<u32>,
    pub energy_saving_mode: EnergySavingMode,
    pub class_of_ms: Option<ClassOfMs>,
}

impl Subscriber {
    pub fn new(issi: u32) -> Self {
        Subscriber {
            issi,
            attached_groups: HashSet::new(),
            energy_saving_mode: EnergySavingMode::StayAlive,
            class_of_ms: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SubscriberStore {
    /// All locally registered MSes, keyed by ISSI
    subscribers: HashMap<u32, Subscriber>,
    /// All groups that currently have locally attached MSes
    attached_groups: HashSet<u32>,
    // Likely remotely registered MSes
    // remote_subscribers: HashSet<u32>,
    // Groups that likely have remotely attached MSes
    // remote_groups: HashSet<u32>,
}

impl SubscriberStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_registered(&self, issi: u32) -> bool {
        self.subscribers.contains_key(&issi)
    }

    /// Registers a new subscriber with default settings. Overwrites existing subscriber if already registered
    pub fn register(&mut self, issi: u32) {
        // To be sure, do clean remove
        self.deregister(issi);
        self.subscribers.insert(issi, Subscriber::new(issi));
    }

    /// Gets a mut ref to a subscriber.
    fn get_subscriber_mut(&mut self, issi: u32) -> Option<&mut Subscriber> {
        self.subscribers.get_mut(&issi)
    }

    /// Gets a ref to a subscriber.
    fn get_subscriber(&self, issi: u32) -> Option<&Subscriber> {
        self.subscribers.get(&issi)
    }

    /// Deregisters a registered subscriber. Returns None if not found
    pub fn deregister(&mut self, issi: u32) -> Option<Subscriber> {
        if let Some(subscriber) = self.subscribers.remove(&issi) {
            for gssi in &subscriber.attached_groups {
                let still_has_members = self.subscribers.values().any(|s| s.attached_groups.contains(gssi));
                if !still_has_members {
                    self.attached_groups.remove(gssi);
                }
            }
            Some(subscriber)
        } else {
            None
        }
    }

    /// Attaches client to a group.
    /// Returns None if client not registered
    /// Returns Some(true) if newly attached attached
    /// Returns Some(false) if the client was already attached
    pub fn group_attach(&mut self, issi: u32, gssi: u32) -> Option<bool> {
        let already_present = {
            let subscriber = match self.get_subscriber_mut(issi) {
                None => return None,
                Some(s) => s,
            };
            // Add to this subscribers attached group list
            let already_present = subscriber.attached_groups.insert(gssi);
            already_present
        };

        // Add to global attached groups list
        self.attached_groups.insert(gssi);
        Some(!already_present)
    }

    /// Detaches a client from a group
    /// Returns None if client not currently registered or not attached to the group
    /// Returns Some(true)if detached but still other clients remain attached to the gssi
    /// Returns Some(false) if detached and no more clients are attached to the gssi
    pub fn group_detach(&mut self, issi: u32, gssi: u32) -> Option<bool> {
        // Get subscriber
        let subscriber = match self.get_subscriber_mut(issi) {
            None => return None,
            Some(s) => s,
        };

        // Remove from subscriber attached groups
        match subscriber.attached_groups.remove(&gssi) {
            false => return None, // not found
            true => {}
        }

        // Remove from global attached groups if no one left attached
        let still_has_members = self.subscribers.values().any(|s| s.attached_groups.contains(&gssi));
        if !still_has_members {
            self.attached_groups.remove(&gssi);
        };
        Some(still_has_members)
    }

    /// Detaches a subscriber from all attached groups.
    /// Returns None if subscriber not registered
    /// Returns Some(true) if successful
    pub fn group_detach_all(&mut self, issi: u32) -> Option<bool> {
        // Get subscriber
        let subscriber = match self.get_subscriber_mut(issi) {
            None => return None,
            Some(s) => s,
        };

        // Get copy of groups
        let groups = subscriber.attached_groups.clone();

        // Detach from all
        for gssi in groups {
            let _ = self.group_detach(issi, gssi);
        }

        Some(true)
    }

    /// Gets a copy of the groups to which user is attached
    pub fn get_attached_groups(&self, issi: u32) -> Option<HashSet<u32>> {
        // Get subscriber
        let subscriber = match self.get_subscriber(issi) {
            None => return None,
            Some(s) => s,
        };

        Some(subscriber.attached_groups.clone())
    }

    /// The group has LOCAL attached members
    pub fn group_has_local_attached_mses(&self, gssi: u32) -> bool {
        self.attached_groups.contains(&gssi)
    }
}
