//! Device modules: the spec engine for spec-driven devices, and a hand-written
//! module for each native one. A native module or native extension is
//! compiled only with the feature of the integration (spec id) that uses it,
//! the same feature that embeds the spec (build.rs), so a spec in the
//! catalogue always has its module and no other integration's code is built.
//! A module shared by several integrations (one protocol, several product
//! lines) is compiled when any of them is.

#[cfg(feature = "adder-aim")]
mod adder_aim;
#[cfg(feature = "aes67-sap")]
mod aes67_sap;
#[cfg(any(feature = "aes70", feature = "focusrite-rednet", feature = "dbaudio"))]
mod aes70;
#[cfg(any(feature = "aja-helo", feature = "aja-kipro", feature = "aja-kumo"))]
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
#[cfg(feature = "apc-rack-pdu")]
mod apc_rack_pdu_profile;
#[cfg(feature = "apc-ups")]
mod apc_ups_profile;
#[cfg(feature = "artnet")]
mod artnet;
#[cfg(feature = "blackmagic-atem")]
mod atem;
#[cfg(feature = "barco-projection-protocol")]
mod barco_projection;
#[cfg(feature = "barco-projection-protocol")]
mod barco_projection_table;
#[cfg(feature = "bss-london")]
mod bss_london;
#[cfg(feature = "christie-pandorasbox")]
mod christie_pandorasbox;
#[cfg(any(feature = "cyberpower-pdu", feature = "cyberpower-ups"))]
mod cps_traps;
#[cfg(feature = "cyberpower-pdu")]
mod cyberpower_pdu_profile;
#[cfg(feature = "cyberpower-ups")]
mod cyberpower_ups_profile;
#[cfg(feature = "datavideo-switcher")]
mod datavideo_switcher;
#[cfg(feature = "datavideo-switcher")]
mod datavideo_switcher_table;
#[cfg(feature = "dbaudio")]
mod dbaudio;
#[cfg(feature = "dhd-ecp")]
mod dhd_ecp;
#[cfg(feature = "digitalprojection-insight-laser")]
mod dp_insight;
#[cfg(feature = "softouch-easyworship")]
mod easyworship;
#[cfg(feature = "emberplus")]
mod emberplus;
#[cfg(feature = "epson-projector")]
mod epson_projector;
#[cfg(feature = "epson-projector")]
mod epson_projector_commands;
#[cfg(feature = "focusrite-rednet")]
mod focusrite_rednet;
#[cfg(feature = "blackmagic-hyperdeck")]
mod ftp;
#[cfg(feature = "generic-http")]
mod generic_http;
#[cfg(feature = "generic-midi")]
mod generic_midi;
#[cfg(feature = "generic-ntp")]
mod generic_ntp;
#[cfg(any(feature = "generic-osc", feature = "osc-listener"))]
mod generic_osc;
#[cfg(feature = "generic-ping")]
mod generic_ping;
#[cfg(feature = "generic-snmp")]
mod generic_snmp;
#[cfg(all(test, feature = "generic-snmp"))]
mod generic_snmp_session_tests;
#[cfg(any(
    feature = "generic-tcp-udp",
    feature = "generic-osc",
    feature = "http-listener",
    feature = "osc-listener"
))]
mod generic_tcp_udp;
#[cfg(feature = "generic-websocket")]
mod generic_websocket;
#[cfg(feature = "http-listener")]
mod http_listener;
#[cfg(feature = "http-snapshot")]
mod http_snapshot;
#[cfg(feature = "blackmagic-hyperdeck")]
mod hyperdeck_ftp;
#[cfg(feature = "integ-jnior")]
mod jnior;
#[cfg(feature = "kissbox-io")]
mod kissbox;
#[cfg(feature = "labgruppen-lake")]
mod lake_dlm;
#[cfg(feature = "meros-camera")]
mod meros_camera;
#[cfg(feature = "meros-camera")]
mod meros_camera_wire;
#[cfg(any(feature = "generic-midi", feature = "midi-show-control"))]
mod midi_link;
#[cfg(feature = "midi-show-control")]
mod midi_show_control;
#[cfg(feature = "generic-modbus")]
mod modbus;
#[cfg(feature = "generic-mqtt")]
mod mqtt;
#[cfg(feature = "nec-display")]
mod nec_display;
#[cfg(feature = "nec-projector")]
mod nec_projector;
#[cfg(feature = "nec-projector")]
mod nec_projector_models;
#[cfg(feature = "nmos-registry")]
mod nmos_query;
#[cfg(feature = "novastar-central-control")]
mod novastar_ccp;
#[cfg(feature = "novastar-h")]
mod novastar_h;
#[cfg(feature = "novastar-h")]
mod novastar_h_commands;
#[cfg(feature = "obs-studio")]
mod obs;
#[cfg(any(feature = "omt", feature = "omt-discovery"))]
mod omt;
#[cfg(feature = "obsidian-onyx")]
mod onyx;
#[cfg(feature = "osc-listener")]
mod osc_listener;
#[cfg(feature = "panasonic-ptz")]
mod panasonic_notify;
#[cfg(feature = "panasonic-projector")]
mod panasonic_projector;
#[cfg(feature = "panasonic-projector")]
mod panasonic_projector_commands;
#[cfg(feature = "christie-pandorasbox")]
mod pandorasbox_commands;
#[cfg(feature = "philips-sicp")]
mod philips_sicp;
#[cfg(feature = "piclock")]
mod piclock;
#[cfg(feature = "pigpio")]
mod pigpio;
#[cfg(feature = "pjlink")]
mod pjlink;
#[cfg(feature = "posistagenet")]
mod posistagenet;
#[cfg(any(feature = "apc-rack-pdu", feature = "apc-ups"))]
mod powernet_traps;
#[cfg(feature = "ptp")]
mod ptp;
#[cfg(feature = "qsys")]
mod qsys;
#[cfg(feature = "middle-atlantic-racklink")]
mod racklink;
#[cfg(feature = "resolume")]
mod resolume_push;
#[cfg(any(feature = "generic-midi", feature = "midi-show-control"))]
mod rtp_journal;
#[cfg(any(feature = "generic-midi", feature = "midi-show-control"))]
mod rtp_midi;
#[cfg(feature = "sacn")]
mod sacn;
#[cfg(feature = "samsung-mdc")]
mod samsung_mdc;
#[cfg(feature = "samsung-mdc")]
mod samsung_mdc_table;
#[cfg(feature = "sennheiser-digital-6000")]
mod sennheiser_d6000;
#[cfg(feature = "sennheiser-ew-dx")]
mod sennheiser_ewdx;
#[cfg(feature = "sennheiser-ew-dx")]
mod sennheiser_ewdx_legacy;
#[cfg(feature = "sennheiser-ew-g3-g4")]
mod sennheiser_mcp;
#[cfg(feature = "sennheiser-spectera")]
mod sennheiser_spectera;
#[cfg(feature = "shure-wireless")]
mod shure;
#[cfg(any(
    feature = "generic-snmp",
    feature = "apc-rack-pdu",
    feature = "apc-ups",
    feature = "cyberpower-pdu",
    feature = "cyberpower-ups",
    feature = "ups-mib"
))]
mod snmp;
#[cfg(any(
    feature = "generic-snmp",
    feature = "apc-rack-pdu",
    feature = "apc-ups",
    feature = "cyberpower-pdu",
    feature = "cyberpower-ups",
    feature = "ups-mib"
))]
mod snmp_ber;
#[cfg(any(
    feature = "apc-rack-pdu",
    feature = "apc-ups",
    feature = "cyberpower-pdu",
    feature = "cyberpower-ups",
    feature = "ups-mib"
))]
mod snmp_device;
#[cfg(all(test, feature = "apc-rack-pdu"))]
mod snmp_device_session_tests;
#[cfg(any(
    feature = "generic-snmp",
    feature = "apc-rack-pdu",
    feature = "apc-ups",
    feature = "cyberpower-pdu",
    feature = "cyberpower-ups",
    feature = "ups-mib"
))]
mod snmp_usm;
#[cfg(feature = "sony-bravia-serial")]
mod sony_bravia_serial;
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
#[cfg(any(feature = "sennheiser-ew-dx", feature = "sennheiser-spectera"))]
mod sscv2;
#[cfg(feature = "probel-swp02")]
mod swp02;
#[cfg(feature = "probel-swp08")]
mod swp08;
#[cfg(feature = "tally-arbiter")]
mod tally_arbiter;
#[cfg(feature = "biamp-tesira")]
mod tesira;
#[cfg(feature = "timemachines-clock")]
mod timemachines;
#[cfg(any(feature = "tsl-umd-display", feature = "tsl-umd-listener"))]
mod tsl;
#[cfg(feature = "blackmagic-ultimatte")]
mod ultimatte_presets;
#[cfg(feature = "blackmagic-ultimatte")]
mod ultimatte_zip;
#[cfg(feature = "ups-mib")]
mod ups_mib_profile;
#[cfg(feature = "vdcp")]
mod vdcp;
#[cfg(feature = "visca")]
mod visca;
#[cfg(feature = "vmix")]
mod vmix;
#[cfg(feature = "vmix")]
mod vmix_functions;
#[cfg(any(feature = "sony-9pin", feature = "grassvalley-amp"))]
mod vtr;
#[cfg(feature = "behringer-wing")]
mod wing_meters;
#[cfg(any(feature = "generic-ping", feature = "samsung-mdc"))]
mod wol;

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
        // An extension that adds commands runs whether or not the device is
        // monitored.
        #[cfg(feature = "blackmagic-ultimatte")]
        if extension == Some("ultimatte-preset-archive") {
            return Ok(Box::new(ultimatte_presets::UltimattePresets::new(
                engine, &context,
            )));
        }
        #[cfg(feature = "blackmagic-hyperdeck")]
        if extension == Some("hyperdeck-ftp") {
            return Ok(Box::new(hyperdeck_ftp::HyperdeckFtp::new(engine, &context)));
        }
        // The other native extensions only keep state current, so a device
        // opened for commands only runs on the engine alone.
        let extension = if context.monitor { extension } else { None };
        return match extension {
            None => Ok(Box::new(engine)),
            #[cfg(feature = "panasonic-ptz")]
            Some("panasonic-update-notification") => Ok(Box::new(
                panasonic_notify::PanasonicNotify::new(engine, &context),
            )),
            #[cfg(any(feature = "aja-helo", feature = "aja-kipro", feature = "aja-kumo"))]
            Some("aja-config-events") => Ok(Box::new(aja_config_events::AjaConfigEvents::new(
                engine, spec, &context,
            )?)),
            #[cfg(feature = "resolume")]
            Some("resolume-push") => Ok(Box::new(resolume_push::ResolumePush::new(
                engine, spec, &context,
            ))),
            #[cfg(feature = "behringer-wing")]
            Some("wing-meters") => Ok(Box::new(wing_meters::WingMeters::new(engine, &context))),
            #[cfg(feature = "nmos-registry")]
            Some("nmos-query-websocket") => {
                Ok(Box::new(nmos_query::NmosQuery::new(engine, &context)))
            }
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
        #[cfg(feature = "novastar-central-control")]
        "novastar-central-control" => Ok(Box::new(novastar_ccp::NovastarCcp::new(context))),
        #[cfg(feature = "novastar-h")]
        "novastar-h" => Ok(Box::new(novastar_h::NovastarH::new(context))),
        #[cfg(feature = "obs-studio")]
        "obs-studio" => Ok(Box::new(obs::Obs::new(context))),
        #[cfg(feature = "omt")]
        "omt" => Ok(Box::new(omt::Sender::new(context))),
        #[cfg(feature = "omt-discovery")]
        "omt-discovery" => Ok(Box::new(omt::Discovery::new(context))),
        #[cfg(feature = "obsidian-onyx")]
        "obsidian-onyx" => Ok(Box::new(onyx::Onyx::new(context))),
        #[cfg(feature = "shure-wireless")]
        "shure-wireless" => Ok(Box::new(shure::Shure::new(context))),
        #[cfg(feature = "biamp-tesira")]
        "biamp-tesira" => Ok(Box::new(tesira::Tesira::new(context))),
        #[cfg(feature = "bss-london")]
        "bss-london" => Ok(Box::new(bss_london::London::new(context))),
        #[cfg(feature = "sony-camera")]
        "sony-camera" => Ok(Box::new(sony_camera::SonyCamera::new(context)?)),
        // One switcher per device, told apart by address: a host is needed.
        #[cfg(feature = "tsl-umd-listener")]
        "tsl-umd-listener" if context.host.is_unspecified() => {
            Err("give the switcher's address as host: each switcher is its own device".into())
        }
        // TSL defines no port, so the host must give one.
        #[cfg(feature = "tsl-umd-listener")]
        "tsl-umd-listener" => match context.port {
            Some(port) => Ok(Box::new(tsl::Listener::new(port, &context.model))),
            None => Err("TSL UMD has no standard port: give the port to listen on".into()),
        },
        #[cfg(feature = "aes67-sap")]
        "aes67-sap" => Ok(Box::new(aes67_sap::Aes67Sap::new(context)?)),
        #[cfg(feature = "artnet")]
        "artnet" => Ok(Box::new(artnet::ArtNet::new(context)?)),
        #[cfg(feature = "sacn")]
        "sacn" => Ok(Box::new(sacn::Sacn::new(context)?)),
        #[cfg(feature = "posistagenet")]
        "posistagenet" => Ok(Box::new(posistagenet::Psn::new(context)?)),
        #[cfg(feature = "adder-aim")]
        "adder-aim" => Ok(Box::new(adder_aim::AdderAim::new(context)?)),
        #[cfg(feature = "integ-jnior")]
        "integ-jnior" => Ok(Box::new(jnior::Jnior::new(context)?)),
        #[cfg(feature = "kissbox-io")]
        "kissbox-io" => Ok(Box::new(kissbox::KissBox::new(context)?)),
        #[cfg(feature = "pigpio")]
        "pigpio" => Ok(Box::new(pigpio::Pigpio::new(context)?)),
        #[cfg(feature = "ptp")]
        "ptp" => Ok(Box::new(ptp::Ptp::new(context)?)),
        #[cfg(feature = "osc-listener")]
        "osc-listener" => Ok(Box::new(osc_listener::OscListener::new(context)?)),
        #[cfg(feature = "tsl-umd-display")]
        "tsl-umd-display" => match context.port {
            Some(port) => Ok(Box::new(tsl::Sender::new(&context, port))),
            None => Err("TSL UMD has no standard port: give the display's port".into()),
        },
        #[cfg(feature = "tally-arbiter")]
        "tally-arbiter" => Ok(Box::new(tally_arbiter::TallyArbiter::new(context))),
        #[cfg(feature = "vmix")]
        "vmix" => Ok(Box::new(vmix::Vmix::new(context))),
        #[cfg(feature = "visca")]
        "visca" => Ok(Box::new(visca::Visca::new(context)?)),
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
            Some(port) => Ok(Box::new(aes70::Aes70::checked(context, port)?)),
            None => Err("AES70 (OCP.1) has no standard port: give the device's OCP.1 port".into()),
        },
        // d&b's integrations use 30013 or 50014 by model; the DS10 and DS100
        // need theirs given.
        #[cfg(feature = "dbaudio")]
        "dbaudio" => match context.port.or_else(|| dbaudio::default_port(&context.model)) {
            Some(port) => Ok(Box::new(dbaudio::DbAudio::new(context, port))),
            None => Err("give the device's OCP.1 port: d&b documents none for this model".into()),
        },
        // RedNet's OCP.1 port is advertised, not fixed, so the host must give it.
        #[cfg(feature = "focusrite-rednet")]
        "focusrite-rednet" => match context.port {
            Some(port) => Ok(Box::new(focusrite_rednet::RedNet::new(context, port))),
            None => Err(
                "RedNet has no fixed OCP.1 port: give the port its _oca._tcp record advertises"
                    .into(),
            ),
        },
        #[cfg(feature = "meros-camera")]
        "meros-camera" => Ok(Box::new(meros_camera::MerosCamera::new(context))),
        #[cfg(feature = "labgruppen-lake")]
        "labgruppen-lake" => Ok(Box::new(lake_dlm::LakeDlm::new(context)?)),
        #[cfg(feature = "emberplus")]
        "emberplus" => Ok(Box::new(emberplus::EmberPlus::new(context))),
        // EasyWorship advertises its port over Bonjour, so the host must give it.
        #[cfg(feature = "softouch-easyworship")]
        "softouch-easyworship" => match context.port {
            Some(port) => Ok(Box::new(easyworship::EasyWorship::new(&context, port))),
            None => Err("EasyWorship has no fixed port: give the port it advertises over Bonjour (_ezwremote._tcp)"
                .into()),
        },
        #[cfg(feature = "barco-projection-protocol")]
        "barco-projection-protocol" => Ok(Box::new(barco_projection::BarcoProjection::new(
            context,
        )?)),
        #[cfg(feature = "datavideo-switcher")]
        "datavideo-switcher" => Ok(Box::new(datavideo_switcher::DatavideoSwitcher::new(
            context,
        )?)),
        #[cfg(feature = "dhd-ecp")]
        "dhd-ecp" => Ok(Box::new(dhd_ecp::DhdEcp::new(spec, context)?)),
        #[cfg(feature = "digitalprojection-insight-laser")]
        "digitalprojection-insight-laser" => Ok(Box::new(dp_insight::DpInsight::new(context)?)),
        #[cfg(feature = "nec-display")]
        "nec-display" => Ok(Box::new(nec_display::NecDisplay::new(context)?)),
        #[cfg(feature = "philips-sicp")]
        "philips-sicp" => Ok(Box::new(philips_sicp::PhilipsSicp::new(context)?)),
        #[cfg(feature = "christie-pandorasbox")]
        "christie-pandorasbox" => Ok(Box::new(christie_pandorasbox::PandorasBox::new(context)?)),
        #[cfg(feature = "probel-swp02")]
        "probel-swp02" => Ok(Box::new(swp02::Swp02::new(context)?)),
        #[cfg(feature = "probel-swp08")]
        "probel-swp08" => Ok(Box::new(swp08::Swp08::new(spec, context)?)),
        // The clock connects to the core, told apart by its address.
        #[cfg(feature = "piclock")]
        "piclock" => Ok(Box::new(piclock::PiClock::new(context)?)),
        #[cfg(feature = "pjlink")]
        "pjlink" => Ok(Box::new(pjlink::PjLink::new(context)?)),
        #[cfg(feature = "epson-projector")]
        "epson-projector" => Ok(Box::new(epson_projector::EpsonProjector::new(spec, context)?)),
        #[cfg(feature = "panasonic-projector")]
        "panasonic-projector" => Ok(Box::new(panasonic_projector::PanasonicProjector::new(
            spec, context,
        )?)),
        #[cfg(feature = "nec-projector")]
        "nec-projector" => Ok(Box::new(nec_projector::NecProjector::new(context)?)),
        #[cfg(feature = "samsung-mdc")]
        "samsung-mdc" => Ok(Box::new(samsung_mdc::SamsungMdc::new(context)?)),
        #[cfg(feature = "sony-bravia-serial")]
        "sony-bravia-serial" => Ok(Box::new(sony_bravia_serial::SonyBraviaSerial::new(context)?)),
        #[cfg(feature = "sony-9pin")]
        "sony-9pin" => Ok(Box::new(vtr::Vtr::new(vtr::Dialect::Sony, context)?)),
        #[cfg(feature = "grassvalley-amp")]
        "grassvalley-amp" => Ok(Box::new(vtr::Vtr::new(vtr::Dialect::Amp, context)?)),
        #[cfg(feature = "vdcp")]
        "vdcp" => Ok(Box::new(vdcp::Vdcp::new(context)?)),
        #[cfg(feature = "generic-modbus")]
        "generic-modbus" => Ok(Box::new(modbus::Modbus::new(context)?)),
        #[cfg(feature = "generic-mqtt")]
        "generic-mqtt" => Ok(Box::new(mqtt::Mqtt::new(context)?)),
        #[cfg(feature = "generic-ntp")]
        "generic-ntp" => Ok(Box::new(generic_ntp::GenericNtp::new(context)?)),
        #[cfg(feature = "timemachines-clock")]
        "timemachines-clock" => Ok(Box::new(timemachines::TimeMachines::new(context)?)),
        #[cfg(feature = "generic-osc")]
        "generic-osc" => Ok(Box::new(generic_osc::GenericOsc::new(context)?)),
        #[cfg(feature = "generic-tcp-udp")]
        "generic-tcp-udp" => Ok(Box::new(generic_tcp_udp::GenericTcpUdp::new(context)?)),
        #[cfg(feature = "generic-ping")]
        "generic-ping" => Ok(Box::new(generic_ping::GenericPing::new(context)?)),
        #[cfg(feature = "generic-snmp")]
        "generic-snmp" => Ok(Box::new(generic_snmp::GenericSnmp::new(context)?)),
        #[cfg(feature = "apc-rack-pdu")]
        "apc-rack-pdu" => Ok(Box::new(snmp_device::SnmpDevice::new(
            &apc_rack_pdu_profile::PROFILE,
            context,
        )?)),
        #[cfg(feature = "apc-ups")]
        "apc-ups" => Ok(Box::new(snmp_device::SnmpDevice::new(
            &apc_ups_profile::PROFILE,
            context,
        )?)),
        #[cfg(feature = "cyberpower-pdu")]
        "cyberpower-pdu" => Ok(Box::new(snmp_device::SnmpDevice::new(
            &cyberpower_pdu_profile::PROFILE,
            context,
        )?)),
        #[cfg(feature = "cyberpower-ups")]
        "cyberpower-ups" => Ok(Box::new(snmp_device::SnmpDevice::new(
            &cyberpower_ups_profile::PROFILE,
            context,
        )?)),
        #[cfg(feature = "ups-mib")]
        "ups-mib" => Ok(Box::new(snmp_device::SnmpDevice::new(
            &ups_mib_profile::PROFILE,
            context,
        )?)),
        #[cfg(feature = "middle-atlantic-racklink")]
        "middle-atlantic-racklink" => Ok(Box::new(racklink::RackLink::new(context)?)),
        #[cfg(feature = "generic-http")]
        "generic-http" => Ok(Box::new(generic_http::GenericHttp::new(context)?)),
        #[cfg(feature = "generic-websocket")]
        "generic-websocket" => Ok(Box::new(generic_websocket::GenericWebsocket::new(
            context,
        )?)),
        #[cfg(feature = "generic-midi")]
        "generic-midi" => Ok(Box::new(generic_midi::GenericMidi::new(context)?)),
        #[cfg(feature = "midi-show-control")]
        "midi-show-control" => Ok(Box::new(midi_show_control::MidiShowControl::new(context)?)),
        #[cfg(feature = "http-listener")]
        "http-listener" => Ok(Box::new(http_listener::HttpListener::new(context)?)),
        #[cfg(feature = "http-snapshot")]
        "http-snapshot" => Ok(Box::new(http_snapshot::HttpSnapshot::new(context)?)),
        #[cfg(feature = "sennheiser-ew-dx")]
        "sennheiser-ew-dx" => Ok(Box::new(sennheiser_ewdx::EwdxReceiver::from_context(
            context,
        ))),
        #[cfg(feature = "sennheiser-spectera")]
        "sennheiser-spectera" => Ok(Box::new(sennheiser_spectera::Spectera::from_context(
            context,
        ))),
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

#[cfg(all(test, feature = "all"))]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use serde_json::Value;

    use super::construct;
    use crate::catalog::{Catalog, Implementation, ParamType, Params};
    use crate::module::{Action, Cx, OpenContext};

    /// The port the module's first outgoing connection or message goes to.
    fn first_port(actions: &[Action]) -> Option<u16> {
        let from_url = |url: &str| {
            let authority = url.split("://").nth(1)?.split('/').next()?;
            authority.rsplit(':').next()?.parse().ok()
        };
        actions.iter().find_map(|a| match a {
            Action::TcpOpen { to, .. } | Action::UdpSend { to, .. } => Some(to.port()),
            // A device that connects to the core: the port listened on.
            Action::TcpListen { port, .. } => Some(*port),
            Action::TcpOpenTls { target, .. } => Some(target.to.port()),
            Action::TcpOpenSsh { tunnel, .. } => Some(tunnel.ssh.port()),
            Action::Http { request, .. } | Action::SseOpen { request, .. } => {
                from_url(&request.url)
            }
            Action::WsOpen { request, .. } => from_url(&request.url),
            _ => None,
        })
    }

    /// Every native module, opened with no port, uses the control port its
    /// spec declares, so the catalogue's defaults are the ones in force.
    #[test]
    fn native_modules_use_the_control_port_their_spec_declares() {
        let catalog = Catalog::source_tree();
        let mut checked = 0;
        for spec in catalog.devices.values() {
            if spec.implementation != Implementation::Native {
                continue;
            }
            // Only the unconditional default can be checked without choosing
            // settings or a model.
            let Some(declared) = spec
                .ports
                .iter()
                .find(|p| p.role == "control" && p.when.is_none())
            else {
                continue;
            };
            let model = &spec.models[0];
            let mut settings = Params::new();
            for (name, setting) in &spec.settings {
                // A plausible value where there is no default, since some
                // modules need one (a console's MIDI channel).
                let value = match (&setting.default, setting.kind) {
                    (Some(default), _) => default.clone(),
                    (None, ParamType::Int) => Value::from(setting.min.unwrap_or(1.0) as i64),
                    (None, ParamType::Float) => Value::from(setting.min.unwrap_or(0.0)),
                    (None, ParamType::Bool) => Value::Bool(false),
                    (None, ParamType::Enum) => {
                        match setting.values.as_ref().and_then(|v| v.first()) {
                            Some(v) => Value::String(v.clone()),
                            None => continue,
                        }
                    }
                    (None, ParamType::String) if setting.required => {
                        Value::String(if name.contains("path") { "/x" } else { "x" }.into())
                    }
                    (None, _) => continue,
                };
                settings.insert(name.clone(), value);
            }
            let context = OpenContext {
                host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
                host_name: None,
                port: None,
                model: model.id.clone(),
                channels: model.channels,
                settings,
                monitor: true,
            };
            match (declared.port, construct(spec, context)) {
                (None, Err(_)) => {}
                (None, Ok(_)) => panic!(
                    "{}: declares no default port but opens without one",
                    spec.id
                ),
                (Some(port), Ok(mut module)) => {
                    let mut cx = Cx::new(0);
                    module.start(&mut cx);
                    let used = first_port(&cx.take());
                    // These send nothing until asked (a request, a watched
                    // stream, a stream to announce), so there is nothing to
                    // compare on starting. An sACN source sends nothing
                    // before it holds a universe, nor a PSN server before it
                    // has a tracker; a PTP monitor only listens.
                    if used.is_none()
                        && [
                            "aes67-sap",
                            "generic-http",
                            "http-snapshot",
                            "posistagenet",
                            "ptp",
                            "sacn",
                        ]
                        .contains(&spec.id.as_str())
                    {
                        continue;
                    }
                    assert_eq!(
                        used,
                        Some(port),
                        "{}: the module's default port differs from its spec's",
                        spec.id
                    );
                    checked += 1;
                }
                (Some(_), Err(e)) => panic!("{}: cannot be opened with defaults: {e}", spec.id),
            }
        }
        assert!(checked >= 20, "only {checked} native modules checked");
    }
}
