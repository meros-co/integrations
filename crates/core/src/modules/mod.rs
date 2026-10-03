//! Device modules: the spec engine for spec-driven devices, and a hand-written
//! module for each native one. A native module or native extension is
//! compiled only with the feature of the integration (spec id) that uses it,
//! the same feature that embeds the spec (build.rs), so a spec in the
//! catalogue always has its module and no other integration's code is built.
//! A module shared by several integrations (one protocol, several product
//! lines) is compiled when any of them is.

#[cfg(feature = "aes70")]
mod aes70;
#[cfg(any(feature = "aja-kipro", feature = "aja-kumo"))]
mod aja_config_events;
#[cfg(any(
    feature = "allenheath-ahm",
    feature = "allenheath-cq",
    feature = "allenheath-dlive",
    feature = "allenheath-qu",
    feature = "allenheath-sq"
))]
mod allenheath;
#[cfg(feature = "allenheath-ahm")]
mod allenheath_ahm;
#[cfg(feature = "allenheath-dlive")]
mod allenheath_dlive;
#[cfg(any(
    feature = "allenheath-ahm",
    feature = "allenheath-cq",
    feature = "allenheath-dlive",
    feature = "allenheath-qu",
    feature = "allenheath-sq"
))]
mod allenheath_midi;
#[cfg(feature = "allenheath-qu")]
mod allenheath_qu;
#[cfg(any(feature = "allenheath-sq", feature = "allenheath-cq"))]
mod allenheath_sq;
#[cfg(any(
    feature = "analogway-alta4k",
    feature = "analogway-livepremier",
    feature = "analogway-midra4k"
))]
mod analogway;
#[cfg(feature = "blackmagic-atem")]
mod atem;
#[cfg(feature = "emberplus")]
mod emberplus;
#[cfg(feature = "generic-http")]
mod generic_http;
#[cfg(feature = "generic-osc")]
mod generic_osc;
#[cfg(any(feature = "generic-tcp-udp", feature = "generic-osc"))]
mod generic_tcp_udp;
#[cfg(feature = "http-snapshot")]
mod http_snapshot;
#[cfg(feature = "obs-studio")]
mod obs;
#[cfg(feature = "obsidian-onyx")]
mod onyx;
#[cfg(feature = "panasonic-ptz")]
mod panasonic_notify;
#[cfg(feature = "pjlink")]
mod pjlink;
#[cfg(feature = "qsys")]
mod qsys;
#[cfg(feature = "resolume")]
mod resolume_push;
#[cfg(feature = "sennheiser-digital-6000")]
mod sennheiser_d6000;
#[cfg(feature = "sennheiser-ew-dx")]
mod sennheiser_ewdx;
#[cfg(feature = "sennheiser-ew-g3-g4")]
mod sennheiser_mcp;
#[cfg(feature = "shure-wireless")]
mod shure;
#[cfg(feature = "sony-camera")]
mod sony_camera;
#[cfg(feature = "sony-camera")]
mod sony_camera_content;
#[cfg(feature = "sony-camera")]
mod sony_camera_dataset;
#[cfg(feature = "sony-camera")]
mod sony_camera_ftp;
#[cfg(feature = "sony-camera")]
mod sony_camera_http;
#[cfg(feature = "sony-camera")]
mod sony_camera_props;
#[cfg(feature = "sony-camera")]
mod sony_camera_ptpip;
#[cfg(any(feature = "tsl-umd-display", feature = "tsl-umd-listener"))]
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
        // The native extensions only keep state current, so a device opened
        // for commands only runs on the engine alone.
        let extension = if context.monitor { extension } else { None };
        return match extension {
            None => Ok(Box::new(engine)),
            #[cfg(feature = "panasonic-ptz")]
            Some("panasonic-update-notification") => Ok(Box::new(
                panasonic_notify::PanasonicNotify::new(engine, &context),
            )),
            #[cfg(any(feature = "aja-kipro", feature = "aja-kumo"))]
            Some("aja-config-events") => Ok(Box::new(aja_config_events::AjaConfigEvents::new(
                engine, spec, &context,
            )?)),
            #[cfg(feature = "resolume")]
            Some("resolume-push") => Ok(Box::new(resolume_push::ResolumePush::new(
                engine, spec, &context,
            ))),
            Some(other) => Err(format!("no native extension '{other}'")),
        };
    }
    match spec.id.as_str() {
        #[cfg(feature = "sennheiser-ew-g3-g4")]
        "sennheiser-ew-g3-g4" => Ok(Box::new(sennheiser_mcp::Mcp::new(context))),
        #[cfg(feature = "sennheiser-digital-6000")]
        "sennheiser-digital-6000" => Ok(Box::new(sennheiser_d6000::D6000::new(context))),
        #[cfg(feature = "blackmagic-atem")]
        "blackmagic-atem" => Ok(Box::new(atem::Atem::new(context))),
        #[cfg(feature = "obs-studio")]
        "obs-studio" => Ok(Box::new(obs::Obs::new(context))),
        #[cfg(feature = "obsidian-onyx")]
        "obsidian-onyx" => Ok(Box::new(onyx::Onyx::new(context))),
        #[cfg(feature = "shure-wireless")]
        "shure-wireless" => Ok(Box::new(shure::Shure::new(context))),
        #[cfg(feature = "sony-camera")]
        "sony-camera" => Ok(Box::new(sony_camera::SonyCamera::new(context)?)),
        // TSL defines no port, so the host must give one.
        #[cfg(feature = "tsl-umd-listener")]
        "tsl-umd-listener" => match context.port {
            Some(port) => Ok(Box::new(tsl::Listener::new(port, &context.model))),
            None => Err("TSL UMD has no standard port: give the port to listen on".into()),
        },
        #[cfg(feature = "tsl-umd-display")]
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
        #[cfg(any(
            feature = "analogway-alta4k",
            feature = "analogway-livepremier",
            feature = "analogway-midra4k"
        ))]
        "analogway-livepremier" | "analogway-midra4k" | "analogway-alta4k" => {
            let dialect = analogway::Dialect::for_spec(&spec.id).expect("an AWJ spec");
            Ok(Box::new(analogway::AnalogWay::new(dialect, context)))
        }
        // OCP.1 has no assigned port, so the host must give the device's.
        #[cfg(feature = "aes70")]
        "aes70" => match context.port {
            Some(port) => Ok(Box::new(aes70::Aes70::new(context, port))),
            None => Err("AES70 (OCP.1) has no standard port: give the device's OCP.1 port".into()),
        },
        #[cfg(feature = "emberplus")]
        "emberplus" => Ok(Box::new(emberplus::EmberPlus::new(context))),
        #[cfg(feature = "pjlink")]
        "pjlink" => Ok(Box::new(pjlink::PjLink::new(context)?)),
        #[cfg(feature = "generic-osc")]
        "generic-osc" => Ok(Box::new(generic_osc::GenericOsc::new(context)?)),
        #[cfg(feature = "generic-tcp-udp")]
        "generic-tcp-udp" => Ok(Box::new(generic_tcp_udp::GenericTcpUdp::new(context)?)),
        #[cfg(feature = "generic-http")]
        "generic-http" => Ok(Box::new(generic_http::GenericHttp::new(context)?)),
        #[cfg(feature = "http-snapshot")]
        "http-snapshot" => Ok(Box::new(http_snapshot::HttpSnapshot::new(context)?)),
        #[cfg(feature = "sennheiser-ew-dx")]
        "sennheiser-ew-dx" => Ok(Box::new(sennheiser_ewdx::Ewdx::new(context))),
        #[cfg(any(
            feature = "allenheath-ahm",
            feature = "allenheath-cq",
            feature = "allenheath-dlive",
            feature = "allenheath-qu",
            feature = "allenheath-sq"
        ))]
        "allenheath-dlive" | "allenheath-ahm" | "allenheath-qu" | "allenheath-sq"
        | "allenheath-cq" => Ok(Box::new(allenheath::AllenHeath::new(&spec.id, context)?)),
        other => Err(format!("no native module for '{other}'")),
    }
}
