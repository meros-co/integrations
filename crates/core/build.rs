//! Embeds the specs in `specs/` whose integrations the enabled features select.
//!
//! A release is a fixed set of specs: nothing is loaded from disk at runtime, so
//! two consumers on the same version and features cannot be running different
//! specs. Every spec is one integration, built in by the Cargo feature named
//! after its spec id (`sennheiser-ew-dx`, `sony-camera`, ...), which also
//! compiles its native module. Each spec also belongs to exactly one vendor
//! group below, a convenience feature (`vendor-sennheiser`) that enables the
//! vendor's integrations; `all` enables every vendor group. A spec missing
//! from this table fails the build, so a new spec cannot be silently left out.

use std::{collections::BTreeSet, env, fs, path::PathBuf};

/// Each vendor group and the integrations (spec ids, which are also their
/// feature names) it enables. Native modules are compiled under the spec id's
/// feature (`src/modules/mod.rs`). `crates/core/Cargo.toml` has one feature
/// per spec id and one per group listing exactly these; a test checks it.
const VENDOR_GROUPS: &[(&str, &[&str])] = &[
    ("vendor-7thsense", &["7thsense-delta"]),
    ("vendor-ableton", &["ableton-live"]),
    ("vendor-adder", &["adder-aim", "adder-xdip"]),
    ("vendor-aes70", &["aes70"]),
    ("vendor-aimedia", &["aimedia-icap"]),
    (
        "vendor-aja",
        &["aja-bridge-live", "aja-helo", "aja-kipro", "aja-kumo"],
    ),
    (
        "vendor-allenheath",
        &[
            "allenheath-ahm",
            "allenheath-cq",
            "allenheath-dlive",
            "allenheath-qu",
            "allenheath-sq",
        ],
    ),
    ("vendor-amwa", &["nmos-node", "nmos-registry"]),
    (
        "vendor-analogway",
        &[
            "analogway-alta4k",
            "analogway-livecore",
            "analogway-livepremier",
            "analogway-midra",
            "analogway-midra4k",
            "analogway-picturall",
        ],
    ),
    ("vendor-anomes", &["millumin"]),
    ("vendor-apc", &["apc-rack-pdu", "apc-ups"]),
    ("vendor-arkaos", &["arkaos-mediamaster"]),
    ("vendor-artisticlicence", &["artnet"]),
    (
        "vendor-atlona",
        &["atlona-matrix", "atlona-ms52w", "atlona-sw510w-rest"],
    ),
    ("vendor-avolites", &["avolites-titan"]),
    ("vendor-avproedge", &["avproedge-matrix"]),
    ("vendor-avstumpfl", &["avstumpfl-pixera"]),
    ("vendor-aws", &["aws-elemental-live"]),
    ("vendor-axia", &["axia-lwrp"]),
    ("vendor-aximmetry", &["aximmetry"]),
    ("vendor-axis", &["axis-ptz"]),
    (
        "vendor-barco",
        &[
            "barco-encore",
            "barco-eventmaster",
            "barco-eventmaster-telnet",
            "barco-projection-protocol",
            "barco-pulse",
            "barco-rlm-w14",
        ],
    ),
    (
        "vendor-behringer",
        &["behringer-wing", "behringer-x32", "behringer-xair"],
    ),
    ("vendor-biamp", &["biamp-tesira"]),
    ("vendor-birddog", &["birddog", "birddog-converters"]),
    (
        "vendor-blackmagic",
        &[
            "blackmagic-atem",
            "blackmagic-audio-monitor",
            "blackmagic-camera",
            "blackmagic-hyperdeck",
            "blackmagic-multiview",
            "blackmagic-smartview",
            "blackmagic-streaming",
            "blackmagic-teranex",
            "blackmagic-ultimatte",
            "blackmagic-videohub",
            "blackmagic-videohub-leitch",
        ],
    ),
    ("vendor-bluenviron", &["mediamtx"]),
    ("vendor-blustream", &["blustream-matrix"]),
    ("vendor-boinx", &["boinx-mimolive"]),
    ("vendor-boxcast", &["boxcast"]),
    ("vendor-brightsign", &["brightsign-dws", "brightsign-udp"]),
    ("vendor-brompton", &["brompton-tessera"]),
    ("vendor-bss", &["bss-london"]),
    ("vendor-camstreamer", &["camstreamer"]),
    ("vendor-canon", &["canon-ptz"]),
    ("vendor-casparcg", &["casparcg"]),
    ("vendor-castr", &["castr"]),
    ("vendor-chamsys", &["chamsys-magicq", "chamsys-magicq-udp"]),
    ("vendor-characterworks", &["characterworks"]),
    (
        "vendor-christie",
        &[
            "christie-1dlp",
            "christie-pandorasbox",
            "christie-spyder",
            "christie-trulife",
        ],
    ),
    ("vendor-churchapps", &["freeshow"]),
    ("vendor-chyron", &["chyron-prime"]),
    ("vendor-clock8001", &["clock-8001"]),
    ("vendor-cockos", &["cockos-reaper", "cockos-reaper-web"]),
    ("vendor-cyberpower", &["cyberpower-pdu", "cyberpower-ups"]),
    ("vendor-dahua", &["dahua-ptz"]),
    ("vendor-dandugan", &["dandugan-model-e"]),
    ("vendor-dashare", &["dashare-multiplay"]),
    (
        "vendor-dataton",
        &["dataton-watchout6", "dataton-watchout7"],
    ),
    ("vendor-datavideo", &["datavideo-switcher"]),
    ("vendor-dbaudio", &["dbaudio"]),
    (
        "vendor-denon",
        &[
            "denon-dn500av",
            "denon-dn500bd",
            "denon-dn500cb",
            "denon-dn500r",
        ],
    ),
    ("vendor-dev-core", &["mixingstation"]),
    ("vendor-dhd", &["dhd-aes67", "dhd-control-api", "dhd-ecp"]),
    ("vendor-digico", &["digico-sd"]),
    (
        "vendor-digitalprojection",
        &[
            "digitalprojection-dvision",
            "digitalprojection-evision-6500",
            "digitalprojection-evision-op",
            "digitalprojection-insight-laser",
            "digitalprojection-projector",
        ],
    ),
    ("vendor-disguise", &["disguise"]),
    ("vendor-dsan", &["dsan-limitimer", "dsan-perfectcue"]),
    ("vendor-dtvideolabs", &["dtvideolabs-playbackpro-plus"]),
    ("vendor-eeg", &["eeg-smart-encoder"]),
    ("vendor-epiphan", &["epiphan-pearl"]),
    ("vendor-epson", &["epson-projector"]),
    ("vendor-esta", &["sacn"]),
    ("vendor-etc", &["etc-echo", "etc-eos", "etc-paradigm"]),
    ("vendor-evertz", &["evertz-quartz"]),
    (
        "vendor-extron",
        &["extron-ipl", "extron-matrix", "extron-switcher"],
    ),
    ("vendor-ezcoo", &["ezcoo-matrix"]),
    ("vendor-faithlife", &["logos-proclaim"]),
    ("vendor-figure53", &["figure53-go-button", "qlab"]),
    (
        "vendor-focusrite",
        &["focusrite-rednet", "focusrite-rednet-rcp"],
    ),
    ("vendor-foscam", &["foscam-ptz"]),
    (
        "vendor-generic",
        &[
            "aes67-sap",
            "generic-http",
            "generic-midi",
            "generic-modbus",
            "generic-mqtt",
            "generic-ntp",
            "generic-osc",
            "generic-ping",
            "generic-snmp",
            "generic-tcp-udp",
            "generic-websocket",
            "http-listener",
            "http-snapshot",
            "midi-show-control",
            "osc-listener",
            "ups-mib",
        ],
    ),
    ("vendor-google", &["youtube-live"]),
    (
        "vendor-grassvalley",
        &["grassvalley-amp", "grassvalley-kaleido"],
    ),
    ("vendor-greenhippo", &["greenhippo-hippotizer"]),
    ("vendor-h2r", &["h2r-graphics"]),
    ("vendor-haivision", &["haivision-kb", "haivision-makito-x4"]),
    ("vendor-highend", &["highend-hog4"]),
    ("vendor-holographics", &["holographics"]),
    ("vendor-holyrics", &["holyrics"]),
    ("vendor-ieee", &["ptp"]),
    ("vendor-imagine", &["vdcp"]),
    ("vendor-imimot", &["imimot-mitti"]),
    ("vendor-inoage", &["madrix"]),
    ("vendor-integ", &["integ-jnior"]),
    ("vendor-ioversal", &["ioversal-vertex"]),
    (
        "vendor-irisdown",
        &["irisdown-countdown-timer", "irisdown-remote-show-control"],
    ),
    ("vendor-joy", &["joy-playdeck"]),
    ("vendor-jvc", &["jvc-camera"]),
    (
        "vendor-kiloview",
        &[
            "kiloview-avx",
            "kiloview-cube-r1",
            "kiloview-cube-x1",
            "kiloview-decoder",
            "kiloview-e3",
            "kiloview-kilolink-server",
            "kiloview-mg300",
            "kiloview-multiview",
            "kiloview-multiview-server",
            "kiloview-n60",
            "kiloview-ndi",
            "kiloview-ndi-core",
            "kiloview-p3",
            "kiloview-rf02",
        ],
    ),
    ("vendor-kissbox", &["kissbox-io"]),
    ("vendor-kramer", &["kramer-p3000"]),
    (
        "vendor-labgruppen",
        &["labgruppen-lake", "labgruppen-nlb60e"],
    ),
    ("vendor-lawo", &["emberplus"]),
    ("vendor-lectrosonics", &["lectrosonics-aspen"]),
    ("vendor-lg", &["lg-display"]),
    ("vendor-lightkey", &["lightkey"]),
    ("vendor-lightware", &["lightware-lw2", "lightware-lw3"]),
    ("vendor-luminex", &["luminex-gigacore"]),
    ("vendor-lyntec", &["lyntec-rpc"]),
    ("vendor-ma-lighting", &["grandma2", "grandma3"]),
    (
        "vendor-magewell",
        &[
            "magewell-director",
            "magewell-proconvert",
            "magewell-ultra-encode",
            "magewell-ultra-stream",
        ],
    ),
    (
        "vendor-matrox",
        &[
            "matrox-monarch-hd",
            "matrox-monarch-hdx",
            "matrox-monarch-lcs",
        ],
    ),
    ("vendor-megapixel", &["megapixel-helios"]),
    ("vendor-meros", &["meros-camera"]),
    ("vendor-meta", &["facebook-account", "facebook-live"]),
    (
        "vendor-meyersound",
        &["meyersound-galaxy", "meyersound-spacemap-go"],
    ),
    ("vendor-middleatlantic", &["middle-atlantic-racklink"]),
    (
        "vendor-modulopi",
        &["modulopi-kinetic", "modulopi-player", "modulopi-spydog"],
    ),
    ("vendor-motu", &["motu-avb"]),
    (
        "vendor-nec",
        &["nec-display", "nec-projector", "nec-projector-ascii"],
    ),
    ("vendor-newblue", &["newblue-captivate"]),
    ("vendor-newtek", &["newtek-3play", "newtek-tricaster"]),
    (
        "vendor-novastar",
        &["novastar-central-control", "novastar-coex", "novastar-h"],
    ),
    ("vendor-obs", &["obs-studio"]),
    ("vendor-obsbot", &["obsbot-tail"]),
    ("vendor-obsidian", &["obsidian-onyx", "obsidian-onyx-osc"]),
    ("vendor-omt", &["omt", "omt-discovery"]),
    ("vendor-ontime", &["ontime", "ontime-osc"]),
    ("vendor-onvif", &["onvif-ptz"]),
    ("vendor-openlp", &["openlp"]),
    ("vendor-opensong", &["opensong"]),
    (
        "vendor-panasonic",
        &[
            "panasonic-kairos",
            "panasonic-projector",
            "panasonic-ptz",
            "panasonic-ptz-serial",
        ],
    ),
    ("vendor-phabrix", &["phabrix-qx"]),
    ("vendor-pharos", &["pharos-designer"]),
    ("vendor-philips", &["philips-sicp"]),
    ("vendor-piclock", &["piclock"]),
    ("vendor-pigpio", &["pigpio"]),
    ("vendor-pikvm", &["pikvm"]),
    ("vendor-pixotope", &["pixotope"]),
    ("vendor-pjlink", &["pjlink"]),
    ("vendor-planningcenter", &["planningcenter-services"]),
    ("vendor-pltech", &["kuma-timer"]),
    ("vendor-posistagenet", &["posistagenet"]),
    ("vendor-presentationtools", &["cuetimer"]),
    ("vendor-probel", &["probel-swp02", "probel-swp08"]),
    ("vendor-ptzoptics", &["ptzoptics"]),
    ("vendor-qlcplus", &["qlcplus"]),
    ("vendor-qsc", &["qsys", "qsys-ecp"]),
    ("vendor-red", &["red-rcp2"]),
    (
        "vendor-renewedvision",
        &["propresenter", "renewedvision-pvp"],
    ),
    ("vendor-resolume", &["resolume"]),
    ("vendor-restream", &["restream"]),
    ("vendor-rme", &["rme-totalmix"]),
    ("vendor-rogueamoeba", &["rogueamoeba-farrago"]),
    (
        "vendor-roland",
        &[
            "roland-m5000",
            "roland-p20hd",
            "roland-v1200hd",
            "roland-v160hd",
            "roland-v600uhd",
            "roland-v60hd",
            "roland-vmixer",
            "roland-vr400uhd",
            "roland-vr4hd",
            "roland-xs1hd",
            "roland-xs42h",
            "roland-xs62s",
            "roland-xs80h",
        ],
    ),
    (
        "vendor-ross",
        &[
            "ross-ultrix",
            "ross-video-server",
            "ross-xpression",
            "ross-xpression-udp",
            "rosstalk",
        ],
    ),
    ("vendor-rundownstudio", &["rundown-studio"]),
    ("vendor-samsung", &["samsung-mdc"]),
    (
        "vendor-sennheiser",
        &[
            "sennheiser-adn",
            "sennheiser-chg-70n",
            "sennheiser-digital-6000",
            "sennheiser-ew-dx",
            "sennheiser-ew-g3-g4",
            "sennheiser-l-6000",
            "sennheiser-spectera",
            "sennheiser-tc-bar",
            "sennheiser-tcc-m",
            "sennheiser-tcc2",
        ],
    ),
    ("vendor-sharp", &["sharp-display"]),
    ("vendor-showcuesystems", &["showcuesystems-scs"]),
    (
        "vendor-shure",
        &[
            "shure-ani",
            "shure-chargers",
            "shure-imx-room",
            "shure-mxa",
            "shure-mxcw",
            "shure-mxn5",
            "shure-mxw",
            "shure-p300",
            "shure-uhfr",
            "shure-wireless",
        ],
    ),
    ("vendor-singular", &["singular-datastream", "singular-live"]),
    ("vendor-snapav", &["snapav-wattbox"]),
    ("vendor-sofie", &["sofie-chef", "sofie-core"]),
    ("vendor-softouch", &["softouch-easyworship"]),
    (
        "vendor-softron",
        &[
            "softron-movierecorder",
            "softron-mreplay",
            "softron-multicam-logger",
            "softron-ontheair-video",
        ],
    ),
    ("vendor-softvelum", &["nimble-streamer", "wmspanel"]),
    ("vendor-songbeamer", &["songbeamer"]),
    (
        "vendor-sony",
        &[
            "sony-9pin",
            "sony-bravia",
            "sony-bravia-ip",
            "sony-bravia-serial",
            "sony-camera",
            "visca",
        ],
    ),
    ("vendor-sounddevices", &["sounddevices-pixnet"]),
    ("vendor-spx", &["spx-graphics"]),
    ("vendor-stagetimer", &["stagetimer"]),
    ("vendor-streamlabs", &["streamlabs-desktop"]),
    ("vendor-studiocoast", &["vmix"]),
    (
        "vendor-symetrix",
        &["symetrix-composer", "symetrix-jupiter"],
    ),
    ("vendor-tallyarbiter", &["tally-arbiter"]),
    (
        "vendor-tascam",
        &[
            "tascam-bd-mp1",
            "tascam-bd-mp4k",
            "tascam-cd-400u",
            "tascam-da6400",
            "tascam-ss-r250n",
        ],
    ),
    ("vendor-teradek", &["teradek-prism"]),
    ("vendor-timemachines", &["timemachines-clock"]),
    ("vendor-tinkerlist", &["cuez-automator"]),
    (
        "vendor-tractus",
        &["tractus-fullview", "tractus-multiview", "tractus-router"],
    ),
    ("vendor-tsl", &["tsl-umd-display", "tsl-umd-listener"]),
    (
        "vendor-turtleav",
        &[
            "turtleav-amp150",
            "turtleav-avoip-control",
            "turtleav-bt-wallplate",
            "turtleav-dante",
            "turtleav-matrix",
        ],
    ),
    ("vendor-tvone", &["tvone-coriomaster"]),
    ("vendor-twitch", &["twitch"]),
    ("vendor-ubiquiti", &["unifi-network"]),
    ("vendor-vaddio", &["vaddio-roboshot"]),
    ("vendor-vdoninja", &["vdoninja"]),
    ("vendor-ventuz", &["ventuz-director"]),
    ("vendor-videolan", &["videolan-vlc"]),
    ("vendor-vimeo", &["vimeo-live"]),
    ("vendor-vioso", &["vioso-exaplay"]),
    ("vendor-visualproductions", &["visualproductions-cuecore"]),
    ("vendor-vizrt", &["vizrt-trio"]),
    ("vendor-workpro", &["workpro-lightshark"]),
    ("vendor-wowza", &["wowza-streaming-engine"]),
    ("vendor-wyrestorm", &["wyrestorm-matrix"]),
    (
        "vendor-yamaha",
        &[
            "yamaha-cl-ql",
            "yamaha-dm3",
            "yamaha-dm7",
            "yamaha-dme7",
            "yamaha-mcp1",
            "yamaha-rivage",
            "yamaha-rm",
            "yamaha-tf",
            "yamaha-vxl",
        ],
    ),
    ("vendor-zcam", &["zcam"]),
    ("vendor-zerodensity", &["zerodensity-reality-hub"]),
    ("vendor-zinc", &["oscpoint"]),
];

fn enabled(feature: &str) -> bool {
    let var = format!(
        "CARGO_FEATURE_{}",
        feature.to_ascii_uppercase().replace('-', "_")
    );
    env::var_os(var).is_some()
}

fn group_of(spec: &str) -> Option<&'static str> {
    VENDOR_GROUPS
        .iter()
        .find(|(_, specs)| specs.contains(&spec))
        .map(|(group, _)| *group)
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let specs_dir = manifest
        .join("../../specs")
        .canonicalize()
        .expect("specs/ directory");
    println!("cargo:rerun-if-changed={}", specs_dir.display());

    let mut entries: Vec<PathBuf> = fs::read_dir(&specs_dir)
        .expect("read specs/")
        .map(|e| e.expect("spec entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    entries.sort();

    let mut seen = BTreeSet::new();
    let mut embedded = String::from("pub(crate) static EMBEDDED_SPECS: &[(&str, &str)] = &[\n");
    let mut all = String::from(
        "/// Every integration (spec id, which is also its feature) in the source\n\
         /// tree, whether this build includes it, and its vendor group.\n\
         pub(crate) static ALL_SPECS: &[(&str, bool, &str)] = &[\n",
    );
    for path in &entries {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        assert!(
            stem != "all" && !stem.starts_with("vendor-"),
            "specs/{stem}.yaml: a spec id cannot be 'all' or start with 'vendor-', \
             which name groups of integrations"
        );
        let group = group_of(&stem).unwrap_or_else(|| {
            panic!(
                "specs/{stem}.yaml is not in the integration table: add it to its vendor \
                 group in VENDOR_GROUPS in crates/core/build.rs, and add a `{stem}` feature \
                 to [features] in crates/core/Cargo.toml (listed in that vendor group)"
            )
        });
        seen.insert(stem.clone());
        let built = enabled(&stem);
        all.push_str(&format!("    ({stem:?}, {built}, {group:?}),\n"));
        if built {
            embedded.push_str(&format!(
                "    ({stem:?}, include_str!({:?})),\n",
                path.display().to_string()
            ));
        }
    }
    embedded.push_str("];\n");
    all.push_str("];\n");

    let mut groups = String::from(
        "/// Every vendor group feature and the integrations it enables.\n\
         pub(crate) static VENDOR_GROUPS: &[(&str, &[&str])] = &[\n",
    );
    let mut listed = BTreeSet::new();
    for (group, specs) in VENDOR_GROUPS {
        assert!(
            group.starts_with("vendor-"),
            "group '{group}' must start with 'vendor-'"
        );
        for spec in *specs {
            assert!(
                seen.contains(*spec),
                "vendor group '{group}' names {spec}, which has no specs/{spec}.yaml"
            );
            assert!(
                listed.insert(*spec),
                "{spec} is in more than one vendor group"
            );
        }
        groups.push_str(&format!("    ({group:?}, &{specs:?}),\n"));
    }
    groups.push_str("];\n");

    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_specs.rs");
    fs::write(dest, format!("{embedded}\n{all}\n{groups}")).expect("write embedded_specs.rs");
}
