//! Device modules: the spec engine for spec-driven devices, and a hand-written
//! module for each native one.

mod sennheiser_d6000;
mod sennheiser_ewdx;
mod sennheiser_mcp;

use std::sync::Arc;

use crate::catalog::{DeviceSpec, Implementation};
use crate::engine::SpecEngine;
use crate::module::{Module, OpenContext};

/// Build the module for a spec. `Err` says why the core cannot drive it.
pub(crate) fn construct(
    spec: &DeviceSpec,
    context: OpenContext,
) -> Result<Box<dyn Module>, String> {
    if spec.implementation == Implementation::Spec {
        return SpecEngine::new(Arc::new(spec.clone()), context)
            .map(|e| Box::new(e) as Box<dyn Module>);
    }
    match spec.id.as_str() {
        "sennheiser-ew-g3-g4" => Ok(Box::new(sennheiser_mcp::Mcp::new(context))),
        "sennheiser-digital-6000" => Ok(Box::new(sennheiser_d6000::D6000::new(context))),
        "sennheiser-ew-dx" => Ok(Box::new(sennheiser_ewdx::Ewdx::new(context))),
        other => Err(format!("no native module for '{other}'")),
    }
}
