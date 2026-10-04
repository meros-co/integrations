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
    ("vendor-aes70", &["aes70"]),
    ("vendor-aja", &["aja-kipro", "aja-kumo"]),
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
    ("vendor-avstumpfl", &["avstumpfl-pixera"]),
    ("vendor-avolites", &["avolites-titan"]),
    ("vendor-barco", &["barco-eventmaster"]),
    (
        "vendor-behringer",
        &["behringer-wing", "behringer-x32", "behringer-xair"],
    ),
    ("vendor-biamp", &["biamp-tesira"]),
    ("vendor-birddog", &["birddog"]),
    (
        "vendor-blackmagic",
        &[
            "blackmagic-atem",
            "blackmagic-camera",
            "blackmagic-hyperdeck",
            "blackmagic-multiview",
            "blackmagic-smartview",
            "blackmagic-streaming",
            "blackmagic-videohub",
        ],
    ),
    ("vendor-brompton", &["brompton-tessera"]),
    ("vendor-canon", &["canon-ptz"]),
    ("vendor-bss", &["bss-london"]),
    ("vendor-chamsys", &["chamsys-magicq", "chamsys-magicq-udp"]),
    ("vendor-christie", &["christie-spyder"]),
    ("vendor-churchapps", &["freeshow"]),
    ("vendor-digico", &["digico-sd"]),
    (
        "vendor-dataton",
        &["dataton-watchout6", "dataton-watchout7"],
    ),
    ("vendor-disguise", &["disguise"]),
    ("vendor-etc", &["etc-eos", "etc-paradigm"]),
    ("vendor-evertz", &["evertz-quartz"]),
    ("vendor-extron", &["extron-matrix", "extron-switcher"]),
    ("vendor-figure53", &["qlab"]),
    (
        "vendor-generic",
        &[
            "generic-http",
            "generic-osc",
            "generic-tcp-udp",
            "http-snapshot",
            "osc-listener",
        ],
    ),
    ("vendor-google", &["youtube-live"]),
    ("vendor-greenhippo", &["greenhippo-hippotizer"]),
    ("vendor-h2r", &["h2r-graphics"]),
    ("vendor-highend", &["highend-hog4"]),
    ("vendor-kramer", &["kramer-p3000"]),
    ("vendor-lawo", &["emberplus"]),
    ("vendor-lightware", &["lightware-lw2", "lightware-lw3"]),
    ("vendor-ma-lighting", &["grandma2", "grandma3"]),
    ("vendor-megapixel", &["megapixel-helios"]),
    ("vendor-newtek", &["newtek-tricaster"]),
    (
        "vendor-novastar",
        &["novastar-central-control", "novastar-coex", "novastar-h"],
    ),
    ("vendor-obs", &["obs-studio"]),
    ("vendor-obsidian", &["obsidian-onyx", "obsidian-onyx-osc"]),
    ("vendor-panasonic", &["panasonic-ptz"]),
    ("vendor-pjlink", &["pjlink"]),
    ("vendor-planningcenter", &["planningcenter-services"]),
    ("vendor-probel", &["probel-swp08"]),
    ("vendor-ptzoptics", &["ptzoptics"]),
    ("vendor-qsc", &["qsys", "qsys-ecp"]),
    (
        "vendor-renewedvision",
        &["propresenter", "renewedvision-pvp"],
    ),
    ("vendor-resolume", &["resolume"]),
    (
        "vendor-roland",
        &[
            "roland-p20hd",
            "roland-v160hd",
            "roland-v600uhd",
            "roland-v60hd",
            "roland-vr400uhd",
            "roland-xs42h",
            "roland-xs62s",
            "roland-xs80h",
        ],
    ),
    (
        "vendor-ross",
        &["ross-xpression", "ross-xpression-udp", "rosstalk"],
    ),
    (
        "vendor-sennheiser",
        &[
            "sennheiser-digital-6000",
            "sennheiser-ew-dx",
            "sennheiser-ew-g3-g4",
        ],
    ),
    (
        "vendor-shure",
        &[
            "shure-ani",
            "shure-imx-room",
            "shure-mxa",
            "shure-mxn5",
            "shure-p300",
            "shure-wireless",
        ],
    ),
    ("vendor-sony", &["sony-camera", "visca"]),
    ("vendor-studiocoast", &["vmix"]),
    (
        "vendor-symetrix",
        &["symetrix-composer", "symetrix-jupiter"],
    ),
    ("vendor-tsl", &["tsl-umd-display", "tsl-umd-listener"]),
    ("vendor-tvone", &["tvone-coriomaster"]),
    (
        "vendor-yamaha",
        &[
            "yamaha-cl-ql",
            "yamaha-dm3",
            "yamaha-dm7",
            "yamaha-dme7",
            "yamaha-rivage",
            "yamaha-rm",
            "yamaha-tf",
        ],
    ),
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
