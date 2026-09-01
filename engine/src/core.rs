pub mod events;

#[derive(Debug, Clone, Copy)]
pub struct PricePoint {
    pub value: f64,
    pub timestamp_ns: u64,
}
