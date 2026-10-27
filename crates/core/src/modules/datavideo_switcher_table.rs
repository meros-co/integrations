//! The controls of Datavideo's processor-unit switchers (SE-1200MU, SE-3200
//! and HS-3200, SE-4000), as their Ethernet Control Protocol documents list
//! them in section 4, "Get & Set Control Commands". Each control is one
//! 32-bit value (an int or a float) named by a control ID: the section in
//! bits 31-16 and the control in bits 15-0, or for the switcher's
//! sub-sections the sub-section in bits 15-8 and the item in bits 7-0, or
//! for an input the input (from 0) in bits 7-4 and the control in bits 3-0.
//!
//! Each control has a state path, which is also the name `set_control` and
//! `get_control` take. Controls a document marks "not used", "no function"
//! or "not supported" on its model are left out, as are the sections the
//! SE-3200 document names but never describes (streamer, clip, logo and
//! animated logo, 12 to 18).

/// A protocol family: the models that share one control table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// SE-1200MU (and the HS-1300, which Datavideo groups with it).
    Se1200,
    /// SE-3200 and HS-3200.
    Se3200,
    /// SE-4000 (and the HS-4000).
    Se4000,
}

/// How a control's 32-bit value reads.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kind {
    Int {
        min: i64,
        max: i64,
    },
    Float {
        min: f64,
        max: f64,
    },
    /// An int, 0 or 1.
    Flag,
    /// A float the document types as such but uses as a flag (key invert).
    FloatFlag,
    /// An int with named values.
    Enum(&'static [(&'static str, i64)]),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Control {
    pub path: String,
    pub id: u32,
    pub kind: Kind,
    pub writable: bool,
}

/// A run of 32-bit words of 1-bit flags, one per stored memory or still.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Present {
    pub path: &'static str,
    pub section: u32,
    pub words: u32,
}

pub(crate) const SECTION_STATUS: u32 = 0;
pub(crate) const SECTION_SYSTEM: u32 = 1;
pub(crate) const SECTION_SWITCHER: u32 = 2;
pub(crate) const SECTION_INPUT: u32 = 3;
pub(crate) const SECTION_INPUT_CTRL: u32 = 4;
pub(crate) const SECTION_OUTPUT_CTRL: u32 = 5;
pub(crate) const SECTION_AUDIO_CTRL: u32 = 6;
pub(crate) const SECTION_TRANSITION_CTRL: u32 = 7;
pub(crate) const SECTION_MEMORY_CTRL: u32 = 8;
pub(crate) const SECTION_MEMORY_PRESENT: u32 = 9;
pub(crate) const SECTION_STILL_CTRL: u32 = 10;
pub(crate) const SECTION_STILL_PRESENT: u32 = 11;
pub(crate) const SECTION_STREAMER_CTRL: u32 = 12;

pub(crate) const fn id(section: u32, control: u32) -> u32 {
    (section << 16) | control
}

const CONNECTION: &[(&str, i64)] = &[("none", 0), ("pending", 1), ("connected", 2)];
const STANDARD_SE1200: &[(&str, i64)] = &[
    ("1080i60", 0),
    ("1080i59.94", 1),
    ("1080i50", 2),
    ("720p60", 3),
    ("720p59.94", 4),
    ("720p50", 5),
];
const STANDARD_SE3200: &[(&str, i64)] = &[
    ("1080p60", 0),
    ("1080p59.94", 1),
    ("1080p50", 2),
    ("1080i60", 3),
    ("1080i59.94", 4),
    ("1080i50", 5),
    ("720p60", 6),
    ("720p59.94", 7),
    ("720p50", 8),
];
const STANDARD_SE4000: &[(&str, i64)] = &[
    ("2160p60", 0),
    ("2160p59.94", 1),
    ("2160p50", 2),
    ("2160p30", 3),
    ("2160p29.97", 4),
    ("2160p25", 5),
    ("2160p24", 6),
    ("2160p23.98", 7),
    ("1080p60", 8),
    ("1080p59.94", 9),
    ("1080p50", 10),
    ("1080p30", 11),
    ("1080p29.97", 12),
    ("1080p25", 13),
    ("1080p24", 14),
    ("1080p23.98", 15),
    ("1080i60", 16),
    ("1080i59.94", 17),
    ("1080i50", 18),
    ("720p60", 19),
    ("720p59.94", 20),
    ("720p50", 21),
];
const KEY_MODE_SE1200: &[(&str, i64)] = &[("luma", 0), ("linear", 1)];
const KEY_MODE: &[(&str, i64)] = &[("luma", 0), ("linear", 1), ("chroma", 2), ("full", 3)];
const DSK_MODE: &[(&str, i64)] = &[("luma", 0), ("linear", 1)];
const KEY_SELECT: &[(&str, i64)] = &[("self", 0), ("split", 1), ("pip", 2)];
const KEY_FILL: &[(&str, i64)] = &[("video", 0), ("matte", 1)];
const TRANSITION_SE1200: &[(&str, i64)] = &[("mix", 0), ("wipe", 1)];
const TRANSITION_SE3200: &[(&str, i64)] = &[("mix", 0), ("wipe", 1), ("clip", 2), ("dve", 3)];
const TRANSITION_SE4000: &[(&str, i64)] = &[("mix", 0), ("wipe", 1), ("clip", 2)];
const KEY_PRIORITY: &[(&str, i64)] = &[("key2_top", 0), ("key1_top", 1)];
const FTB_DIRECTION: &[(&str, i64)] = &[("down", 0), ("up", 1)];
const BORDER_STYLE: &[(&str, i64)] = &[
    ("off", 0),
    ("normal", 1),
    ("shaded", 2),
    ("bevel", 3),
    ("bevel_shaded", 4),
    ("bevel_double", 5),
    ("bevel_flat", 6),
    ("flat_bevel", 7),
    ("glass", 8),
    ("glass_shaded", 9),
    ("glass_double", 10),
    ("glass_flat", 11),
    ("flat_glass", 12),
];
const BORDER_STYLE_SE4000: &[(&str, i64)] = &[("off", 0), ("normal", 1)];
const FREEZE: &[(&str, i64)] = &[
    ("live", 0),
    ("freeze", 1),
    ("still", 2),
    ("clip", 3),
    ("capture", 4),
];
const FRAME_MODE: &[(&str, i64)] = &[("frame", 0), ("field", 1), ("video_and_key", 2)];
const YCK: &[(&str, i64)] = &[("yc", 0), ("yck", 1)];
const RUN_STATE: &[(&str, i64)] = &[
    ("stopped", 0),
    ("at_start", 1),
    ("running", 2),
    ("at_end", 3),
    ("paused", 4),
];
const DVI_OUT: &[(&str, i64)] = &[
    ("program", 0),
    ("preview", 1),
    ("program_dsk1", 2),
    ("preview_dsk1", 3),
    ("multiviewer", 4),
];
const AUDIO_PAIR: &[(&str, i64)] = &[
    ("1_2", 0),
    ("3_4", 1),
    ("5_6", 2),
    ("7_8", 3),
    ("9_10", 4),
    ("11_12", 5),
    ("13_14", 6),
    ("15_16", 7),
];
const AUDIO_MODE_SE1200: &[(&str, i64)] = &[("off", 0), ("digital", 1), ("analog", 2), ("test", 3)];
const AUDIO_MODE: &[(&str, i64)] = &[("off", 0), ("on", 1)];
pub(crate) const TRANS_COMMAND: &[(&str, i64)] = &[
    ("stop", 0),
    ("run", 1),
    ("pause", 2),
    ("continue", 3),
    ("goto_start", 4),
    ("goto_end", 5),
    ("restart", 6),
    ("stop_and_clear", 7),
    ("ready", 8),
];
const TRANS_TYPE: &[(&str, i64)] = &[("one_shot", 0), ("loop", 1), ("ping_pong", 2)];
const DIRECTION: &[(&str, i64)] = &[("forward", 0), ("reverse", 1)];
pub(crate) const MEMORY_COMMAND: &[(&str, i64)] =
    &[("ready", 0), ("load", 1), ("store", 2), ("delete", 3)];
const PROCESSOR_STATE: &[(&str, i64)] = &[("ready", 0), ("busy", 1), ("error", 2)];
const RESULT: &[(&str, i64)] = &[
    ("ok", 0),
    ("fail", 1),
    ("not_found", 2),
    ("illegal_command", 3),
    ("illegal_value", 4),
];
pub(crate) const STILL_COMMAND: &[(&str, i64)] = &[
    ("ready", 0),
    ("load", 1),
    ("store", 2),
    ("grab", 3),
    ("delete", 4),
];
const STREAMER_SIZE: &[(&str, i64)] = &[("full", 0), ("half", 1), ("quarter", 2), ("sixth", 3)];

const fn int(min: i64, max: i64) -> Kind {
    Kind::Int { min, max }
}
const fn float(min: f64, max: f64) -> Kind {
    Kind::Float { min, max }
}
const PERCENT: Kind = float(0.0, 100.0);
const HUE: Kind = float(0.0, 360.0);
const SOURCE: Kind = int(0, 30);
const ANY: Kind = int(i32::MIN as i64, i32::MAX as i64);

struct Table(Vec<Control>);

impl Table {
    fn rw(&mut self, path: impl Into<String>, id: u32, kind: Kind) {
        self.0.push(Control {
            path: path.into(),
            id,
            kind,
            writable: true,
        });
    }
    fn ro(&mut self, path: impl Into<String>, id: u32, kind: Kind) {
        self.0.push(Control {
            path: path.into(),
            id,
            kind,
            writable: false,
        });
    }
}

/// How many inputs, keyers (with or without their own PinP) and aux buses a
/// family has.
pub(crate) fn inputs(family: Family) -> u32 {
    match family {
        Family::Se1200 => 6,
        Family::Se3200 => 12,
        Family::Se4000 => 8,
    }
}

pub(crate) fn keyers(family: Family) -> u32 {
    match family {
        Family::Se1200 => 2,
        Family::Se3200 | Family::Se4000 => 4,
    }
}

/// The 1-bit present flags: user memories, then stills.
pub(crate) fn present(family: Family) -> Vec<Present> {
    vec![
        Present {
            path: "memory.stored",
            section: SECTION_MEMORY_PRESENT,
            words: 32,
        },
        Present {
            path: "stills.stored",
            section: SECTION_STILL_PRESENT,
            words: if family == Family::Se1200 { 4 } else { 32 },
        },
    ]
}

/// Every control of a family, in the order of its document.
pub(crate) fn controls(family: Family) -> Vec<Control> {
    let mut t = Table(Vec::new());
    let se1200 = family == Family::Se1200;
    let s = |c: u32| id(SECTION_STATUS, c);

    // 4.1.1 Status: read only.
    t.ro("status.connection", s(0), Kind::Enum(CONNECTION));
    t.ro("status.system_version", s(1), ANY);
    t.ro("status.main_version", s(2), ANY);
    t.ro("status.software_version", s(3), ANY);
    if se1200 {
        t.ro("status.fpga_version", s(4), ANY);
        t.ro("status.board_version", s(6), ANY);
        for n in 0..20 {
            t.ro(format!("tally.sources.{}", n + 1), s(7 + n), ANY);
        }
        let buses = [
            "program",
            "preview",
            "keyers.1.fill",
            "keyers.1.key",
            "keyers.2.fill",
            "keyers.2.key",
            "dsks.1.fill",
            "dsks.1.key",
            "dsks.2.fill",
            "dsks.2.key",
        ];
        for (n, bus) in buses.iter().enumerate() {
            t.ro(format!("tally.{bus}"), s(27 + n as u32), ANY);
        }
    } else {
        t.ro("status.firmware_version", s(4), ANY);
        t.ro("status.keyboard_version", s(5), ANY);
        t.ro("status.board_version", s(6), ANY);
        t.ro("status.product_version", s(7), ANY);
        t.ro("status.product_subversion", s(8), ANY);
        t.ro("status.boot_status", s(9), ANY);
        for n in 0..20 {
            t.ro(format!("tally.sources.{}", n + 1), s(10 + n), ANY);
        }
        let buses = [
            "program",
            "preview",
            "keyers.1.fill",
            "keyers.1.key",
            "keyers.2.fill",
            "keyers.2.key",
            "keyers.3.fill",
            "keyers.3.key",
            "keyers.4.fill",
            "keyers.4.key",
            "dsks.1.fill",
            "dsks.1.key",
            "dsks.2.fill",
            "dsks.2.key",
        ];
        for (n, bus) in buses.iter().enumerate() {
            t.ro(format!("tally.{bus}"), s(30 + n as u32), ANY);
        }
        t.ro("status.free_space", s(44), ANY);
    }

    // 4.1.2 System.
    let y = |c: u32| id(SECTION_SYSTEM, c);
    let standard = match family {
        Family::Se1200 => STANDARD_SE1200,
        Family::Se3200 => STANDARD_SE3200,
        Family::Se4000 => STANDARD_SE4000,
    };
    t.rw("system.standard", y(0), Kind::Enum(standard));
    t.rw("system.aspect", y(1), Kind::Flag);
    t.rw("system.genlock", y(2), Kind::Flag);
    t.rw("system.genlock_source", y(3), ANY);
    if !se1200 {
        t.rw("system.ip_mode", y(6), ANY);
        t.rw("system.ip_address", y(7), ANY);
        t.rw("system.netmask", y(8), ANY);
        t.rw("system.gateway", y(9), ANY);
        t.ro("system.mac_address_0", y(10), ANY);
        t.ro("system.mac_address_1", y(11), ANY);
        t.rw("system.menu_color", y(14), ANY);
        t.rw("system.menu_position", y(15), ANY);
        t.rw("system.transparent_menus", y(16), ANY);
        t.rw("system.small_menus", y(17), ANY);
        t.rw("system.auto_save", y(18), ANY);
        t.rw("system.language", y(19), ANY);
        t.rw("system.menu_mode", y(20), ANY);
        t.rw("system.keyboard_brightness", y(21), ANY);
        t.rw("system.serial_mode", y(22), Kind::Flag);
        t.rw("system.audio_analog_level", y(23), Kind::Flag);
    }

    // 4.1.3 Switcher, main sub-section.
    let w = |c: u32| id(SECTION_SWITCHER, c);
    t.rw("wipe.pattern", w(0), int(1, 32));
    t.rw("wipe.level", w(1), PERCENT);
    t.rw("wipe.position_x", w(2), float(-16.0, 16.0));
    t.rw("wipe.position_y", w(3), float(-16.0, 16.0));
    t.rw("wipe.rotation", w(4), float(-16.0, 16.0));
    t.rw("wipe.softness", w(5), PERCENT);
    t.rw("wipe.soft_balance", w(6), float(-100.0, 100.0));
    t.rw("wipe.border_width", w(8), PERCENT);
    t.rw("wipe.border", w(9), Kind::Flag);
    t.rw("wipe.border_hue", w(10), HUE);
    t.rw("wipe.border_saturation", w(11), PERCENT);
    t.rw("wipe.border_luma", w(12), PERCENT);

    let key_mode = if se1200 { KEY_MODE_SE1200 } else { KEY_MODE };
    for k in 0..keyers(family) {
        let b = 19 + 30 * k;
        let p = |f: &str| format!("keyers.{}.{f}", k + 1);
        t.rw(p("on"), w(b), Kind::Flag);
        t.rw(p("key_source"), w(b + 1), SOURCE);
        t.rw(p("split_source"), w(b + 2), SOURCE);
        t.rw(p("opacity"), w(b + 3), PERCENT);
        t.rw(p("lift"), w(b + 4), PERCENT);
        t.rw(p("gain"), w(b + 5), float(0.0, 16.0));
        t.rw(p("mode"), w(b + 6), Kind::Enum(key_mode));
        t.rw(p("invert"), w(b + 7), Kind::FloatFlag);
        t.rw(p("select_mode"), w(b + 8), Kind::Enum(KEY_SELECT));
        t.rw(p("fill_mode"), w(b + 9), Kind::Enum(KEY_FILL));
        t.rw(p("matte_hue"), w(b + 10), HUE);
        t.rw(p("matte_saturation"), w(b + 11), PERCENT);
        t.rw(p("matte_luma"), w(b + 12), PERCENT);
        t.rw(p("chroma.enabled"), w(b + 13), Kind::Flag);
        t.rw(p("chroma.hue"), w(b + 14), HUE);
        t.rw(p("chroma.saturation"), w(b + 15), PERCENT);
        t.rw(p("chroma.luma"), w(b + 16), PERCENT);
        t.rw(p("chroma.key_acceptance"), w(b + 17), float(0.0, 180.0));
        t.rw(p("chroma.key_gain"), w(b + 18), float(0.0, 16.0));
        t.rw(p("chroma.key_lift"), w(b + 19), PERCENT);
        t.rw(p("chroma.chroma_acceptance"), w(b + 20), float(0.0, 180.0));
        t.rw(p("chroma.suppression"), w(b + 21), PERCENT);
        t.rw(p("chroma.background_suppress"), w(b + 24), Kind::Flag);
        t.rw(p("mask.left"), w(b + 25), PERCENT);
        t.rw(p("mask.right"), w(b + 26), PERCENT);
        t.rw(p("mask.top"), w(b + 27), PERCENT);
        t.rw(p("mask.bottom"), w(b + 28), PERCENT);
        t.rw(p("mask.enabled"), w(b + 29), Kind::Flag);
    }

    // Transition controls, DSKs, bus matte, FTB and flex source follow the
    // keyers, so their numbers depend on how many keyers there are.
    let base = 19 + 30 * keyers(family);
    t.rw("next_transition.background", w(base), Kind::Flag);
    for k in 0..keyers(family) {
        t.rw(
            format!("next_transition.key{}", k + 1),
            w(base + 1 + k),
            Kind::Flag,
        );
    }
    let b = base + 1 + keyers(family);
    if se1200 {
        t.rw("next_transition.swap_priority", w(b), Kind::Flag);
    }
    t.rw("transition.preview", w(b + 1), Kind::Flag);
    t.rw("transition.reverse", w(b + 2), Kind::Flag);
    t.rw("transition.normal_reverse", w(b + 3), Kind::Flag);
    t.rw("program", w(b + 4), SOURCE);
    t.rw("preview", w(b + 5), SOURCE);
    let types = match family {
        Family::Se1200 => TRANSITION_SE1200,
        Family::Se3200 => TRANSITION_SE3200,
        Family::Se4000 => TRANSITION_SE4000,
    };
    t.rw("transition.type", w(b + 6), Kind::Enum(types));
    if family == Family::Se3200 {
        t.rw("transition.dve_number", w(b + 7), ANY);
    }
    if se1200 {
        t.rw("key_priority", w(b + 8), Kind::Enum(KEY_PRIORITY));
    }
    let dsk = b + 9;
    for d in 0..2 {
        let b = dsk + 18 * d;
        let p = |f: &str| format!("dsks.{}.{f}", d + 1);
        t.rw(p("on"), w(b), Kind::Flag);
        t.rw(p("key_source"), w(b + 1), SOURCE);
        t.rw(p("split_source"), w(b + 2), SOURCE);
        t.rw(p("opacity"), w(b + 3), PERCENT);
        t.rw(p("lift"), w(b + 4), PERCENT);
        t.rw(p("gain"), w(b + 5), float(0.0, 16.0));
        t.rw(p("mode"), w(b + 6), Kind::Enum(DSK_MODE));
        t.rw(p("invert"), w(b + 7), Kind::FloatFlag);
        t.rw(p("select_mode"), w(b + 8), Kind::Enum(KEY_SELECT));
        t.rw(p("fill_mode"), w(b + 9), Kind::Enum(KEY_FILL));
        t.rw(p("matte_hue"), w(b + 10), HUE);
        t.rw(p("matte_saturation"), w(b + 11), PERCENT);
        t.rw(p("matte_luma"), w(b + 12), PERCENT);
        t.rw(p("mask.left"), w(b + 13), PERCENT);
        t.rw(p("mask.right"), w(b + 14), PERCENT);
        t.rw(p("mask.top"), w(b + 15), PERCENT);
        t.rw(p("mask.bottom"), w(b + 16), PERCENT);
        t.rw(p("mask.enabled"), w(b + 17), Kind::Flag);
    }
    let b = dsk + 36;
    t.rw("dsks.1.in_transition", w(b), Kind::Flag);
    t.rw("dsks.2.in_transition", w(b + 1), Kind::Flag);
    t.ro("dsk_transition_level", w(b + 2), PERCENT);
    t.rw("bus_matte.hue", w(b + 3), HUE);
    t.rw("bus_matte.saturation", w(b + 4), PERCENT);
    t.rw("bus_matte.luma", w(b + 5), PERCENT);
    t.rw("ftb.enabled", w(b + 6), Kind::Flag);
    t.rw("ftb.direction", w(b + 7), Kind::Enum(FTB_DIRECTION));
    t.ro("ftb.level", w(b + 8), PERCENT);
    let flex = b + 9;
    match family {
        Family::Se3200 => {
            t.rw("flex.background", w(flex), SOURCE);
            for n in 0..4 {
                t.rw(
                    format!("flex.pinps.{}.source", n + 1),
                    w(flex + 1 + n),
                    SOURCE,
                );
            }
            t.rw("flex.foreground", w(flex + 5), SOURCE);
            t.rw("flex.foreground_key", w(flex + 6), SOURCE);
            t.rw("flex.foreground_enabled", w(flex + 7), Kind::Flag);
        }
        Family::Se1200 => {
            // The SE-1200's single resizable DVE tile, on the flex source's
            // DVE 1 controls (4.1.3.11); its flex source itself is unused.
            let d = flex + 6;
            let fields: [(&str, u32, Kind); 22] = [
                ("position_x", 1, float(-16.0, 16.0)),
                ("position_y", 2, float(-16.0, 16.0)),
                ("position_z", 3, float(-16.0, 16.0)),
                ("size_x", 5, float(0.0, 16.0)),
                ("size_y", 6, float(0.0, 16.0)),
                ("size_z", 7, float(0.0, 16.0)),
                ("crop_size", 8, PERCENT),
                ("crop_left", 9, PERCENT),
                ("crop_right", 10, PERCENT),
                ("crop_top", 11, PERCENT),
                ("crop_bottom", 12, PERCENT),
                ("crop_soft", 13, PERCENT),
                ("border_size", 14, PERCENT),
                ("border_left", 15, PERCENT),
                ("border_right", 16, PERCENT),
                ("border_top", 17, PERCENT),
                ("border_bottom", 18, PERCENT),
                ("border_soft", 19, PERCENT),
                ("border_style", 20, Kind::FloatFlag),
                ("border_hue", 21, HUE),
                ("border_saturation", 22, PERCENT),
                ("border_luma", 23, PERCENT),
            ];
            for (name, offset, kind) in fields {
                t.rw(format!("dve.{name}"), w(d + offset), kind);
            }
        }
        Family::Se4000 => {}
    }

    // 4.1.5 / 4.1.6: the keyers' PinPs, and the flex source's.
    if !se1200 {
        for k in 0..4 {
            pinp(&mut t, family, 1, k, &format!("keyers.{}.pinp", k + 1));
        }
        if family == Family::Se3200 {
            for k in 0..4 {
                pinp(&mut t, family, 2, k, &format!("flex.pinps.{}", k + 1));
            }
        }
    }

    // 4.1.7 Inputs: the input in bits 7-4.
    for n in 0..inputs(family) {
        let i = |c: u32| id(SECTION_INPUT, (n << 4) | c);
        let p = |f: &str| format!("inputs.{}.{f}", n + 1);
        t.rw(p("black_level"), i(0), PERCENT);
        t.rw(p("chroma_gain"), i(1), float(0.0, 16.0));
        t.rw(p("white_clip"), i(2), PERCENT);
        t.ro(p("valid"), i(3), Kind::Flag);
        let o = if se1200 {
            0
        } else {
            t.ro(p("standard"), i(4), ANY);
            1
        };
        t.rw(p("freeze"), i(5 + o), Kind::Enum(FREEZE));
        t.rw(p("frame_mode"), i(6 + o), Kind::Enum(FRAME_MODE));
        t.rw(p("remap"), i(7 + o), int(0, inputs(family) as i64));
        t.rw(p("still_load"), i(8 + o), Kind::Flag);
        t.rw(p("still_number"), i(9 + o), ANY);
        if !se1200 {
            t.rw(p("audio_gain"), i(11), float(-60.0, 24.0));
            t.rw(p("audio_delay"), i(12), int(0, 340));
            t.rw(p("audio_remap"), i(13), int(0, inputs(family) as i64));
        }
    }

    // 4.1.8 Common input controls.
    let c = |n: u32| id(SECTION_INPUT_CTRL, n);
    if se1200 {
        t.rw("input_ctrl.dvi_input", c(0), Kind::Flag);
        t.rw("input_ctrl.remap", c(1), Kind::Flag);
    } else {
        t.ro("media.clip.valid", c(4), Kind::Flag);
        t.rw("media.clip.number", c(5), ANY);
        t.rw("media.clip.yck", c(6), Kind::Enum(YCK));
        t.ro("media.clip.frames", c(7), ANY);
        t.rw("media.clip.state", c(8), Kind::Enum(RUN_STATE));
        t.rw("media.clip.frame", c(9), ANY);
        for n in 0..2 {
            t.ro(
                format!("media.stills.{}.valid", n + 1),
                c(16 + 2 * n),
                Kind::Flag,
            );
            t.rw(format!("media.stills.{}.number", n + 1), c(17 + 2 * n), ANY);
        }
        for n in 0..2 {
            t.ro(
                format!("media.logos.{}.valid", n + 1),
                c(20 + 3 * n),
                Kind::Flag,
            );
            t.rw(
                format!("media.logos.{}.animated", n + 1),
                c(21 + 3 * n),
                Kind::Flag,
            );
            t.rw(format!("media.logos.{}.number", n + 1), c(22 + 3 * n), ANY);
        }
    }

    // 4.1.9 Outputs.
    let o = |n: u32| id(SECTION_OUTPUT_CTRL, n);
    if se1200 {
        t.rw("multiviewer.main_1", o(5), SOURCE);
    } else {
        for n in 0..4 {
            t.rw(format!("aux.{}.source", n + 1), o(n), SOURCE);
        }
        t.rw("multiviewer.mode", o(4), ANY);
        t.rw("multiviewer.main_1", o(5), SOURCE);
        t.rw("multiviewer.main_2", o(6), SOURCE);
    }
    t.rw("multiviewer.transparent_labels", o(9), Kind::Flag);
    t.rw("multiviewer.auto_number", o(10), Kind::Flag);
    t.rw("multiviewer.label_info", o(11), Kind::Flag);
    if se1200 {
        t.rw("outputs.analog.source", o(12), ANY);
        t.rw("outputs.analog.mode", o(13), ANY);
        t.rw("outputs.analog.sync_mode", o(14), ANY);
        t.rw("outputs.dvi.source", o(15), Kind::Enum(DVI_OUT));
        t.rw("outputs.multi_1.source", o(16), Kind::Enum(DVI_OUT));
    } else {
        for n in 0..3 {
            t.rw(format!("outputs.hdmi.{}.source", n + 1), o(15 + n), SOURCE);
        }
        for n in 0..2 {
            t.rw(format!("outputs.sdi.{}.source", n + 1), o(18 + n), SOURCE);
        }
        t.rw("gpi_out.enabled", o(20), Kind::Flag);
        t.rw("gpi_out.mode", o(21), ANY);
        t.rw("gpi_out.source", o(22), ANY);
        t.rw("gpi_out.delay", o(23), ANY);
        t.rw("gpi_out.width", o(24), ANY);
        for n in 0..3 {
            t.rw(format!("outputs.hdmi.{}.mode_1080", n + 1), o(25 + n), ANY);
        }
        t.rw("tally_mode", o(28), ANY);
    }

    // 4.1.10 Audio.
    let a = |n: u32| id(SECTION_AUDIO_CTRL, n);
    if se1200 {
        t.rw("audio.source", a(0), int(0, 4));
        t.rw("audio.channels", a(1), Kind::Enum(AUDIO_PAIR));
        t.rw("audio.mode", a(2), Kind::Enum(AUDIO_MODE_SE1200));
    } else {
        t.rw("audio.source", a(0), int(0, 13));
        t.rw("audio.mode", a(2), Kind::Enum(AUDIO_MODE));
        t.rw("audio.analog_level", a(3), ANY);
        t.rw("audio.out_source", a(4), ANY);
        t.rw("audio.in_mode", a(5), ANY);
        t.rw("audio.out_mode", a(6), ANY);
        t.rw("audio.mixer_mode", a(7), ANY);
        t.rw("audio.master_gain", a(8), float(-60.0, 24.0));
        t.rw("audio.master_delay", a(9), ANY);
        t.rw("audio.out_gain", a(10), float(-60.0, 24.0));
        t.rw("audio.out_delay", a(11), ANY);
        for n in 0..6 {
            t.rw(format!("audio.sdi.{}", n + 1), a(12 + n), Kind::Flag);
        }
        for n in 0..3 {
            t.rw(format!("audio.hdmi.{}", n + 1), a(18 + n), Kind::Flag);
        }
        for n in 0..12 {
            t.rw(
                format!("audio.inputs.{}.gain", n + 1),
                a(21 + 2 * n),
                float(-60.0, 24.0),
            );
            t.rw(format!("audio.inputs.{}.delay", n + 1), a(22 + 2 * n), ANY);
        }
    }

    // 4.1.11 Transition engines: M/E, DSK, FTB.
    for (e, engine) in ["me", "dsk", "ftb"].iter().enumerate() {
        let x = |n: u32| id(SECTION_TRANSITION_CTRL, 5 * e as u32 + n);
        let p = |f: &str| format!("transitions.{engine}.{f}");
        t.rw(p("command"), x(0), Kind::Enum(TRANS_COMMAND));
        t.rw(p("type"), x(1), Kind::Enum(TRANS_TYPE));
        t.ro(p("state"), x(2), Kind::Enum(RUN_STATE));
        t.rw(p("duration"), x(3), int(0, 9999));
        t.rw(p("direction"), x(4), Kind::Enum(DIRECTION));
    }

    // 4.1.12 User memories.
    let m = |n: u32| id(SECTION_MEMORY_CTRL, n);
    t.rw("memory.select", m(0), int(0, 1000));
    t.rw("memory.command", m(1), Kind::Enum(MEMORY_COMMAND));
    t.ro("memory.state", m(2), Kind::Enum(PROCESSOR_STATE));
    t.ro("memory.result", m(3), Kind::Enum(RESULT));
    t.ro("memory.event", m(4), ANY);
    t.rw("memory.flags", m(5), ANY);
    t.rw("memory.load_all_sections", m(6), Kind::Flag);

    // 4.1.14 Stills.
    let st = |n: u32| id(SECTION_STILL_CTRL, n);
    t.rw("stills.select", st(0), int(0, 999));
    t.rw("stills.buffer", st(1), int(0, 7));
    t.rw("stills.command", st(2), Kind::Enum(STILL_COMMAND));
    t.ro("stills.state", st(3), Kind::Enum(PROCESSOR_STATE));
    t.ro("stills.result", st(4), Kind::Enum(RESULT));
    t.ro("stills.event", st(5), ANY);

    // 4.1.13 (SE-1200) Streamer: only the size is implemented.
    if se1200 {
        t.rw(
            "streamer.size",
            id(SECTION_STREAMER_CTRL, 2),
            Kind::Enum(STREAMER_SIZE),
        );
    }
    t.0
}

/// One PinP's 37 controls (SE-4000: the first 25, without border opacity).
fn pinp(t: &mut Table, family: Family, sub: u32, n: u32, prefix: &str) {
    let w = |item: u32| id(SECTION_SWITCHER, (sub << 8) | (37 * n + item));
    let p = |f: &str| format!("{prefix}.{f}");
    let full = family == Family::Se3200;
    t.rw(p("enabled"), w(0), Kind::Flag);
    t.rw(p("position_x"), w(1), float(-16.0, 16.0));
    t.rw(p("position_y"), w(2), float(-16.0, 16.0));
    t.rw(p("position_z"), w(3), float(-16.0, 16.0));
    t.rw(p("size_x"), w(5), float(0.0, 16.0));
    t.rw(p("size_y"), w(6), float(0.0, 16.0));
    t.rw(p("size_z"), w(7), float(0.0, 16.0));
    t.rw(p("crop_size"), w(8), PERCENT);
    t.rw(p("crop_left"), w(9), PERCENT);
    t.rw(p("crop_right"), w(10), PERCENT);
    t.rw(p("crop_top"), w(11), PERCENT);
    t.rw(p("crop_bottom"), w(12), PERCENT);
    t.rw(p("crop_soft"), w(13), PERCENT);
    let styles = if full {
        BORDER_STYLE
    } else {
        BORDER_STYLE_SE4000
    };
    t.rw(p("border_style"), w(14), Kind::Enum(styles));
    if full {
        t.rw(p("border_opacity"), w(15), PERCENT);
    }
    t.rw(p("border_size"), w(16), PERCENT);
    t.rw(p("border_left"), w(17), PERCENT);
    t.rw(p("border_right"), w(18), PERCENT);
    t.rw(p("border_top"), w(19), PERCENT);
    t.rw(p("border_bottom"), w(20), PERCENT);
    t.rw(p("border_soft"), w(21), PERCENT);
    t.rw(p("border_hue"), w(22), HUE);
    t.rw(p("border_saturation"), w(23), PERCENT);
    t.rw(p("border_luma"), w(24), PERCENT);
    if full {
        t.rw(p("shade_hue"), w(25), HUE);
        t.rw(p("shade_saturation"), w(26), PERCENT);
        t.rw(p("shade_luma"), w(27), PERCENT);
        t.rw(p("shade_soft"), w(28), PERCENT);
        t.rw(p("shade_position"), w(29), PERCENT);
        t.rw(p("shade_corner_soft"), w(30), PERCENT);
        t.rw(p("border_direction"), w(31), ANY);
        t.rw(p("highlight_hue"), w(32), HUE);
        t.rw(p("highlight_saturation"), w(33), PERCENT);
        t.rw(p("highlight_luma"), w(34), PERCENT);
        t.rw(p("highlight_opacity"), w(35), PERCENT);
        t.rw(p("lowlight_opacity"), w(36), PERCENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_the_documents_examples() {
        let se3200 = controls(Family::Se3200);
        let find = |p: &str| se3200.iter().find(|c| c.path == p).unwrap().id;
        // 5.3: PGM 0x94, PVW 0x95, key 1 on 0x13, FTB enable 195, the
        // transition type 0x96, M/E duration 7/3, FTB command 7/10.
        assert_eq!(find("program"), 0x0002_0094);
        assert_eq!(find("preview"), 0x0002_0095);
        assert_eq!(find("keyers.1.on"), 0x0002_0013);
        assert_eq!(find("ftb.enabled"), 0x0002_00C3);
        assert_eq!(find("transition.type"), 0x0002_0096);
        assert_eq!(find("transitions.me.duration"), 0x0007_0003);
        assert_eq!(find("transitions.ftb.command"), 0x0007_000A);
        assert_eq!(find("memory.select"), 0x0008_0000);
        // 4.1.3: DSK 1 on is 153, key 4 mask enable 138, flex BGND 198.
        assert_eq!(find("dsks.1.on"), 0x0002_0099);
        assert_eq!(find("keyers.4.mask.enabled"), 138 | 0x0002_0000);
        assert_eq!(find("flex.background"), 198 | 0x0002_0000);
        assert_eq!(find("flex.foreground_enabled"), 205 | 0x0002_0000);
        // 4.1.4.1: key 1 PinP position X is 0x00020101.
        assert_eq!(find("keyers.1.pinp.position_x"), 0x0002_0101);
        assert_eq!(find("keyers.2.pinp.enabled"), 0x0002_0125);
        // 4.1.7.1: input 7 freeze mode is 0x00030066.
        assert_eq!(find("inputs.7.freeze"), 0x0003_0066);

        let se1200 = controls(Family::Se1200);
        let find = |p: &str| se1200.iter().find(|c| c.path == p).unwrap().id;
        assert_eq!(find("next_transition.background"), 79 | 0x0002_0000);
        assert_eq!(find("program"), 86 | 0x0002_0000);
        assert_eq!(find("key_priority"), 90 | 0x0002_0000);
        assert_eq!(find("dsks.1.on"), 91 | 0x0002_0000);
        assert_eq!(find("dsks.2.mask.enabled"), 126 | 0x0002_0000);
        assert_eq!(find("ftb.level"), 135 | 0x0002_0000);
        assert_eq!(find("dve.position_x"), 143 | 0x0002_0000);
        assert_eq!(find("dve.border_luma"), 165 | 0x0002_0000);
        assert_eq!(find("tally.dsks.2.key"), 36);
        assert_eq!(find("inputs.2.freeze"), 0x0003_0015);
    }

    #[test]
    fn paths_and_ids_are_unique() {
        for family in [Family::Se1200, Family::Se3200, Family::Se4000] {
            let all = controls(family);
            let mut paths = std::collections::BTreeSet::new();
            let mut ids = std::collections::BTreeSet::new();
            for c in &all {
                assert!(paths.insert(c.path.clone()), "{family:?}: {}", c.path);
                assert!(ids.insert(c.id), "{family:?}: {} {:08x}", c.path, c.id);
            }
        }
    }
}
