//! Device modules: the spec engine for spec-driven devices, and a hand-written
//! module for each native one.

mod atem;
mod obs;
mod panasonic_notify;
mod sennheiser_d6000;
mod sennheiser_ewdx;
mod sennheiser_mcp;
mod shure;
mod tsl;
mod visca;
mod vmix;
mod vmix_functions;

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
        let extension = spec.extension.as_deref();
        let engine = SpecEngine::new(Arc::new(spec.clone()), context.clone())?;
        return match extension {
            None => Ok(Box::new(engine)),
            Some("panasonic-update-notification") => Ok(Box::new(
                panasonic_notify::PanasonicNotify::new(engine, &context),
            )),
            Some(other) => Err(format!("no native extension '{other}'")),
        };
    }
    match spec.id.as_str() {
        "sennheiser-ew-g3-g4" => Ok(Box::new(sennheiser_mcp::Mcp::new(context))),
        "sennheiser-digital-6000" => Ok(Box::new(sennheiser_d6000::D6000::new(context))),
        "blackmagic-atem" => Ok(Box::new(atem::Atem::new(context))),
        "obs-studio" => Ok(Box::new(obs::Obs::new(context))),
        "shure-wireless" => Ok(Box::new(shure::Shure::new(context))),
        // TSL defines no port, so the host must give one.
        "tsl-umd-listener" => match context.port {
            Some(port) => Ok(Box::new(tsl::Listener::new(port, &context.model))),
            None => Err("TSL UMD has no standard port: give the port to listen on".into()),
        },
        "tsl-umd-display" => match context.port {
            Some(port) => Ok(Box::new(tsl::Sender::new(&context, port))),
            None => Err("TSL UMD has no standard port: give the display's port".into()),
        },
        "vmix" => Ok(Box::new(vmix::Vmix::new(context))),
        "visca" => Ok(Box::new(visca::Visca::new(context))),
        "sennheiser-ew-dx" => Ok(Box::new(sennheiser_ewdx::Ewdx::new(context))),
        other => Err(format!("no native module for '{other}'")),
    }
}
