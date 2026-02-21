#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrewSubscriberAction {
    Register,
    Deregister,
    Affiliate,
    Deaffiliate,
}

#[derive(Debug, Clone)]
pub struct BrewSubscriberUpdate {
    pub issi: u32,
    pub groups: Vec<u32>,
    pub action: BrewSubscriberAction,
}
