//! Device modules: the spec engine for spec-driven devices, and a hand-written
//! module for each native one. A native module is compiled only with its
//! feature family (build.rs), the same feature that embeds its spec, so a
//! spec in the catalogue always has its module.

#[cfg(feature = "aja")]
mod aja_config_events;
#[cfg(feature = "allenheath")]
mod allenheath;
#[cfg(feature = "allenheath")]
mod allenheath_ahm;
#[cfg(feature = "allenheath")]
mod allenheath_dlive;
#[cfg(feature = "allenheath")]
mod allenheath_midi;
#[cfg(feature = "allenheath")]
mod allenheath_qu;
#[cfg(feature = "allenheath")]
mod allenheath_sq;
#[cfg(feature = "analogway")]
mod analogway;
#[cfg(feature = "blackmagic")]
mod atem;
#[cfg(feature = "emberplus")]
mod emberplus;
#[cfg(feature = "generic")]
mod generic_http;
#[cfg(feature = "generic")]
mod generic_osc;
#[cfg(feature = "generic")]
mod generic_tcp_udp;
#[cfg(feature = "generic")]
mod http_snapshot;
#[cfg(feature = "obs")]
mod obs;
#[cfg(feature = "panasonic")]
mod panasonic_notify;
#[cfg(feature = "pjlink")]
mod pjlink;
#[cfg(feature = "qsys")]
mod qsys;
#[cfg(feature = "sennheiser")]
mod sennheiser_d6000;
#[cfg(feature = "sennheiser")]
mod sennheiser_ewdx;
#[cfg(feature = "sennheiser")]
mod sennheiser_mcp;
#[cfg(feature = "shure")]
mod shure;
#[cfg(feature = "sony")]
mod sony_camera;
#[cfg(feature = "sony")]
mod sony_camera_content;
#[cfg(feature = "sony")]
mod sony_camera_dataset;
#[cfg(feature = "sony")]
mod sony_camera_ftp;
#[cfg(feature = "sony")]
mod sony_camera_http;
#[cfg(feature = "sony")]
mod sony_camera_props;
#[cfg(feature = "sony")]
mod sony_camera_ptpip;
#[cfg(feature = "tsl")]
mod tsl;
#[cfg(feature = "visca")]
mod visca;
#[cfg(feature = "vmix")]
mod vmix;
#[cfg(feature = "vmix")]
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
            #[cfg(feature = "panasonic")]
            Some("panasonic-update-notification") => Ok(Box::new(
                panasonic_notify::PanasonicNotify::new(engine, &context),
            )),
            #[cfg(feature = "aja")]
            Some("aja-config-events") => Ok(Box::new(aja_config_events::AjaConfigEvents::new(
                engine, spec, &context,
            )?)),
            Some(other) => Err(format!("no native extension '{other}'")),
        };
    }
    match spec.id.as_str() {
        #[cfg(feature = "sennheiser")]
        "sennheiser-ew-g3-g4" => Ok(Box::new(sennheiser_mcp::Mcp::new(context))),
        #[cfg(feature = "sennheiser")]
        "sennheiser-digital-6000" => Ok(Box::new(sennheiser_d6000::D6000::new(context))),
        #[cfg(feature = "blackmagic")]
        "blackmagic-atem" => Ok(Box::new(atem::Atem::new(context))),
        #[cfg(feature = "obs")]
        "obs-studio" => Ok(Box::new(obs::Obs::new(context))),
        #[cfg(feature = "shure")]
        "shure-wireless" => Ok(Box::new(shure::Shure::new(context))),
        #[cfg(feature = "sony")]
        "sony-camera" => Ok(Box::new(sony_camera::SonyCamera::new(context)?)),
        // TSL defines no port, so the host must give one.
        #[cfg(feature = "tsl")]
        "tsl-umd-listener" => match context.port {
            Some(port) => Ok(Box::new(tsl::Listener::new(port, &context.model))),
            None => Err("TSL UMD has no standard port: give the port to listen on".into()),
        },
        #[cfg(feature = "tsl")]
        "tsl-umd-display" => match context.port {
            Some(port) => Ok(Box::new(tsl::Sender::new(&context, port))),
            None => Err("TSL UMD has no standard port: give the display's port".into()),
        },
        #[cfg(feature = "vmix")]
        "vmix" => Ok(Box::new(vmix::Vmix::new(context))),
        #[cfg(feature = "visca")]
        "visca" => Ok(Box::new(visca::Visca::new(context))),
        #[cfg(feature = "qsys")]
        "qsys" => Ok(Box::new(qsys::Qsys::new(context))),
        #[cfg(feature = "analogway")]
        "analogway-livepremier" | "analogway-midra4k" | "analogway-alta4k" => {
            let dialect = analogway::Dialect::for_spec(&spec.id).expect("an AWJ spec");
            Ok(Box::new(analogway::AnalogWay::new(dialect, context)))
        }
        #[cfg(feature = "emberplus")]
        "emberplus" => Ok(Box::new(emberplus::EmberPlus::new(context))),
        #[cfg(feature = "pjlink")]
        "pjlink" => Ok(Box::new(pjlink::PjLink::new(context)?)),
        #[cfg(feature = "generic")]
        "generic-osc" => Ok(Box::new(generic_osc::GenericOsc::new(context)?)),
        #[cfg(feature = "generic")]
        "generic-tcp-udp" => Ok(Box::new(generic_tcp_udp::GenericTcpUdp::new(context)?)),
        #[cfg(feature = "generic")]
        "generic-http" => Ok(Box::new(generic_http::GenericHttp::new(context)?)),
        #[cfg(feature = "generic")]
        "http-snapshot" => Ok(Box::new(http_snapshot::HttpSnapshot::new(context)?)),
        #[cfg(feature = "sennheiser")]
        "sennheiser-ew-dx" => Ok(Box::new(sennheiser_ewdx::Ewdx::new(context))),
        #[cfg(feature = "allenheath")]
        "allenheath-dlive" | "allenheath-ahm" | "allenheath-qu" | "allenheath-sq"
        | "allenheath-cq" => Ok(Box::new(allenheath::AllenHeath::new(&spec.id, context)?)),
        other => Err(format!("no native module for '{other}'")),
    }
}
