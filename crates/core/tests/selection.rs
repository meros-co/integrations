//! A core started for some devices: Sennheiser and Shure wireless only. Its
//! catalogue, open and discovery know nothing else.

#![cfg(all(
    feature = "sennheiser-digital-6000",
    feature = "sennheiser-ew-dx",
    feature = "sennheiser-ew-g3-g4",
    feature = "shure-wireless"
))]

use meros_integrations::{
    json as api, Core, CoreOptions, DiscoverAction, DiscoverRequest, OpenError, OpenRequest,
};
use serde_json::json;

const WIRELESS: [&str; 4] = [
    "sennheiser-ew-dx",
    "sennheiser-ew-g3-g4",
    "sennheiser-digital-6000",
    "shure-wireless",
];

fn wireless() -> Core {
    Core::with_options(CoreOptions::new().devices(WIRELESS)).unwrap()
}

fn open(core: &Core, device: &str, model: &str) -> Result<u64, OpenError> {
    core.open(OpenRequest {
        device: device.into(),
        model: model.into(),
        host: "127.0.0.1".into(),
        port: Some(9),
        settings: Default::default(),
        monitor: true,
    })
}

fn discover(protocols: &[&str], action: DiscoverAction) -> DiscoverRequest {
    DiscoverRequest {
        action,
        protocols: protocols.iter().map(|p| p.to_string()).collect(),
        hints: vec![],
    }
}

#[test]
fn the_catalogue_holds_only_the_selected_devices() {
    let core = wireless();
    let mut ids: Vec<&str> = core.catalog().devices.keys().map(String::as_str).collect();
    ids.sort();
    let mut expected = WIRELESS.to_vec();
    expected.sort();
    assert_eq!(ids, expected);
    // The JSON every binding returns is the same catalogue.
    let catalog = api::catalog(&core);
    assert_eq!(
        catalog["devices"].as_object().unwrap().len(),
        WIRELESS.len()
    );
    assert!(catalog["devices"].get("sony-camera").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_selected_device_opens_and_any_other_is_refused() {
    let core = wireless();
    let device = open(&core, "shure-wireless", "ulxd4").unwrap();
    core.close(device).await;

    let every = Core::new().unwrap();
    for (id, spec) in &every.catalog().devices {
        if WIRELESS.contains(&id.as_str()) {
            continue;
        }
        assert_eq!(
            open(&core, id, &spec.models[0].id),
            Err(OpenError::NotSelected { device: id.clone() }),
        );
    }
    assert_eq!(
        open(&core, "no-such-device", "x"),
        Err(OpenError::UnknownDevice {
            device: "no-such-device".into()
        })
    );
    let refused = api::open(
        &core,
        &json!({"device": "kramer-p3000", "model": "p3000-generic", "host": "127.0.0.1"}),
    );
    if every.catalog().device("kramer-p3000").is_some() {
        assert_eq!(refused["error"]["error"], "not_selected");
        assert_eq!(
            refused["error"]["message"],
            "device 'kramer-p3000' is not among the devices this core was started with"
        );
    }
}

#[test]
fn discovery_runs_only_the_protocols_of_the_selected_devices() {
    let core = wireless();
    assert_eq!(core.discovery_protocols(), ["mcp"]);
    for excluded in ["ssdp", "pjlink"] {
        let err = core
            .discover(discover(&[excluded], DiscoverAction::Scan))
            .unwrap_err();
        assert!(err.contains("this core does not include"), "{err}");
    }
    let err = core
        .discover(discover(&["bonjour"], DiscoverAction::Scan))
        .unwrap_err();
    assert!(err.contains("unknown discovery protocol"), "{err}");
    // Empty means every protocol of the selection: here MCP alone.
    core.discover(discover(&[], DiscoverAction::Stop)).unwrap();

    let shure_only = Core::with_options(CoreOptions::new().devices(["shure-wireless"])).unwrap();
    assert!(shure_only.discovery_protocols().is_empty());
    assert!(shure_only
        .discover(discover(&["mcp"], DiscoverAction::Listen))
        .is_err());
    shure_only
        .discover(discover(&[], DiscoverAction::Scan))
        .unwrap();
}

#[test]
fn a_selection_the_build_cannot_meet_fails_construction() {
    for devices in [vec!["no-such-device".to_string()], vec![]] {
        let err = Core::with_options(CoreOptions::new().devices(devices))
            .err()
            .expect("refused");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
    // The JSON options every binding passes.
    let options = api::core_options(&json!({"devices": ["shure-wireless"]})).unwrap();
    assert_eq!(options.devices, Some(vec!["shure-wireless".to_string()]));
    assert!(Core::with_options(options).is_ok());
}

/// A build without the `sony-camera` integration has no Sony camera at all,
/// and says so.
#[cfg(not(feature = "sony-camera"))]
#[test]
fn an_integration_left_out_of_the_build_is_named() {
    let core = Core::new().unwrap();
    assert!(core.catalog().device("sony-camera").is_none());
    assert!(!core.discovery_protocols().contains(&"ssdp"));
    assert_eq!(
        open(&core, "sony-camera", "ilce-7m4"),
        Err(OpenError::NotBuilt {
            device: "sony-camera".into(),
            feature: "sony-camera".into()
        })
    );
    let err = Core::with_options(CoreOptions::new().devices(["sony-camera"]))
        .err()
        .expect("refused")
        .to_string();
    assert!(err.contains("the 'sony-camera' feature"), "{err}");
}

/// A vendor group names every integration of one vendor in the build.
#[test]
fn a_vendor_group_selects_its_integrations() {
    let core =
        Core::with_options(CoreOptions::new().devices(["vendor-sennheiser", "shure-wireless"]))
            .unwrap();
    let mut ids: Vec<&str> = core.catalog().devices.keys().map(String::as_str).collect();
    ids.sort();
    let mut expected = WIRELESS.to_vec();
    // Built in when the build has it; the group always names it.
    if cfg!(feature = "sennheiser-spectera") {
        expected.push("sennheiser-spectera");
    }
    expected.sort();
    assert_eq!(ids, expected);
    let options = api::core_options(&json!({"devices": ["vendor-shure"]})).unwrap();
    let shure = Core::with_options(options).unwrap();
    assert_eq!(
        shure.catalog().devices.keys().collect::<Vec<_>>(),
        [
            "shure-ani",
            "shure-imx-room",
            "shure-mxa",
            "shure-mxn5",
            "shure-p300",
            "shure-wireless"
        ]
    );
    let every = Core::with_options(CoreOptions::new().devices(["all"])).unwrap();
    assert_eq!(
        every.catalog().devices.len(),
        Core::new().unwrap().catalog().devices.len()
    );
}
