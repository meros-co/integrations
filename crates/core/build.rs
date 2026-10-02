//! Embeds the specs in `specs/` that the enabled features select.
//!
//! A release is a fixed set of specs: nothing is loaded from disk at runtime, so
//! two consumers on the same version and features cannot be running different
//! specs. Every spec belongs to exactly one family below, and each family is a
//! Cargo feature of the same name (see Cargo.toml): a product that builds with
//! `default-features = false` gets only the families it names. A spec that is
//! in no family fails the build, so a new spec cannot be silently left out.

use std::{collections::BTreeSet, env, fs, path::PathBuf};

/// Each feature family and the spec ids (file stems) it embeds. Native modules
/// are compiled under the same feature names (`src/modules/mod.rs`).
const FAMILIES: &[(&str, &[&str])] = &[
    ("aja", &["aja-kipro", "aja-kumo"]),
    (
        "allenheath",
        &[
            "allenheath-ahm",
            "allenheath-cq",
            "allenheath-dlive",
            "allenheath-qu",
            "allenheath-sq",
        ],
    ),
    (
        "analogway",
        &[
            "analogway-alta4k",
            "analogway-livecore",
            "analogway-livepremier",
            "analogway-midra",
            "analogway-midra4k",
            "analogway-picturall",
        ],
    ),
    ("barco", &["barco-eventmaster"]),
    (
        "behringer",
        &["behringer-wing", "behringer-x32", "behringer-xair"],
    ),
    ("birddog", &["birddog"]),
    (
        "blackmagic",
        &[
            "blackmagic-atem",
            "blackmagic-camera",
            "blackmagic-hyperdeck",
            "blackmagic-streaming",
            "blackmagic-videohub",
        ],
    ),
    ("chamsys", &["chamsys-magicq", "chamsys-magicq-udp"]),
    ("emberplus", &["emberplus"]),
    ("etc", &["etc-eos"]),
    (
        "generic",
        &[
            "generic-http",
            "generic-osc",
            "generic-tcp-udp",
            "http-snapshot",
        ],
    ),
    ("h2r", &["h2r-graphics"]),
    ("kramer", &["kramer-p3000"]),
    ("ma-lighting", &["grandma2", "grandma3"]),
    ("newtek", &["newtek-tricaster"]),
    ("obs", &["obs-studio"]),
    ("panasonic", &["panasonic-ptz"]),
    ("pjlink", &["pjlink"]),
    ("ptzoptics", &["ptzoptics"]),
    ("qlab", &["qlab"]),
    ("qsys", &["qsys"]),
    ("renewedvision", &["propresenter", "renewedvision-pvp"]),
    ("resolume", &["resolume"]),
    (
        "roland",
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
        "ross",
        &["ross-xpression", "ross-xpression-udp", "rosstalk"],
    ),
    (
        "sennheiser",
        &[
            "sennheiser-digital-6000",
            "sennheiser-ew-dx",
            "sennheiser-ew-g3-g4",
        ],
    ),
    ("shure", &["shure-wireless"]),
    ("sony", &["sony-camera"]),
    ("tsl", &["tsl-umd-display", "tsl-umd-listener"]),
    ("visca", &["visca"]),
    ("vmix", &["vmix"]),
    (
        "yamaha",
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

fn enabled(family: &str) -> bool {
    let var = format!(
        "CARGO_FEATURE_{}",
        family.to_ascii_uppercase().replace('-', "_")
    );
    env::var_os(var).is_some()
}

fn family_of(stem: &str) -> Option<&'static str> {
    FAMILIES
        .iter()
        .find(|(_, specs)| specs.contains(&stem))
        .map(|(family, _)| *family)
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
        "/// Every spec in the source tree, built in or not, and its feature family.\n\
         pub(crate) static ALL_SPECS: &[(&str, &str)] = &[\n",
    );
    for path in &entries {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let family = family_of(&stem).unwrap_or_else(|| {
            panic!(
                "specs/{stem}.yaml belongs to no feature family: add it to FAMILIES in \
                 crates/core/build.rs (and a new family to [features] in crates/core/Cargo.toml)"
            )
        });
        seen.insert(stem.clone());
        all.push_str(&format!("    ({stem:?}, {family:?}),\n"));
        if enabled(family) {
            embedded.push_str(&format!(
                "    ({stem:?}, include_str!({:?})),\n",
                path.display().to_string()
            ));
        }
    }
    embedded.push_str("];\n");
    all.push_str("];\n");
    for (family, specs) in FAMILIES {
        for spec in *specs {
            assert!(
                seen.contains(*spec),
                "family '{family}' names {spec}, which has no specs/{spec}.yaml"
            );
        }
    }

    let mut families = String::from(
        "/// Every feature family and whether this build includes it.\n\
         pub(crate) static FAMILIES: &[(&str, bool)] = &[\n",
    );
    for (family, _) in FAMILIES {
        families.push_str(&format!("    ({family:?}, {}),\n", enabled(family)));
    }
    families.push_str("];\n");

    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_specs.rs");
    fs::write(dest, format!("{embedded}\n{all}\n{families}")).expect("write embedded_specs.rs");
}
