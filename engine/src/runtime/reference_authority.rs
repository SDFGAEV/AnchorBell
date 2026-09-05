use crate::{
    execution::BinanceEnvironment,
    simulation_runtime::{BinanceIndexAnchorSet, SimulationError},
};

/// Single authority for fetching and validating runtime anchor/reference data.
///
/// Consumers must use this port instead of reaching into a simulation or
/// execution implementation. The returned set is immutable for the caller's
/// run and carries the FX provenance used for local-currency reporting.
pub async fn fetch(
    environment: BinanceEnvironment,
    symbols: &[String],
    price_scale: u32,
    http_proxy: Option<&str>,
) -> Result<BinanceIndexAnchorSet, SimulationError> {
    crate::simulation_runtime::load_index_anchor_set_internal(
        environment,
        symbols,
        price_scale,
        http_proxy,
    )
    .await
}
