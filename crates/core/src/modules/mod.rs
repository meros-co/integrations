//! Native device modules, keyed by spec id.

mod sennheiser_d6000;
mod sennheiser_ewdx;
mod sennheiser_mcp;

use crate::module::{Module, OpenContext};

/// Build the module for a spec, or `None` if the core has no implementation
/// for it yet.
pub(crate) fn construct(spec: &str, context: OpenContext) -> Option<Box<dyn Module>> {
    match spec {
        "sennheiser-ew-g3-g4" => Some(Box::new(sennheiser_mcp::Mcp::new(context))),
        "sennheiser-digital-6000" => Some(Box::new(sennheiser_d6000::D6000::new(context))),
        "sennheiser-ew-dx" => Some(Box::new(sennheiser_ewdx::Ewdx::new(context))),
        _ => None,
    }
}
