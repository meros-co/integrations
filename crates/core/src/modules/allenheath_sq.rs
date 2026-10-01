//! SQ, SQ+, Qu-5/6/7 and CQ, which share one protocol: every mute, level,
//! pan and assignment is an NRPN parameter number with a 14-bit value. From
//! the "SQ MIDI Protocol" (Issue 5 for firmware V1.5, and the firmware V1.6
//! article), "SQ+ MIDI Protocol - Firmware V2.0.3", "Qu-5/6/7 MIDI Protocol"
//! (Issue 2) and "CQ MIDI Protocol, Firmware V1.2" (Issue 5). Page numbers are
//! the SQ Issue 5 PDF's unless marked.
//!
//! - `BN 63 MB, BN 62 LB` selects the parameter; `BN 06 VC, BN 26 VF` sets
//!   its value: 00 00 / 00 01 for mutes and assignments, a 14-bit level or
//!   pan otherwise (p.11-17). `BN 60 00` and `BN 61 00` step a level 1 dB or
//!   a pan up or down, and toggle a mute or assignment; `BN 60 7F` asks for
//!   the value, which comes back as the set message (p.18).
//! - The parameter numbers follow one layout for all four families
//!   (p.21-31): a source index (inputs from 00, groups from 30, FX returns
//!   from 3C), mixes (LR 44, aux from 45), 12 auxes per source, 4 FX sends
//!   per source, and matrix sends per mix. The families differ in how many
//!   of each they have, where stereo sources sit, and SQ+'s six matrices and
//!   moved output block; the tables here are checked against the documents'
//!   own values in the tests.
//! - Levels follow the console's "NRPN Fader Law": linear taper (the
//!   default) or audio taper, each with a table in the document (p.20). CQ
//!   has one table, the audio taper's.
//! - Scenes are bank and Program Change: banks 00-02 for scenes 1-300 and,
//!   on SQ, 0A-0C for cue list positions (p.9). The console sends the same
//!   message when a scene is recalled on it.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::allenheath::{invalid, midi_channel, setting_flag, unknown, Args, Dialect, Plan};
use super::allenheath_midi::{self as midi, level_updates, Event, Law, Update};
use crate::catalog::Params;
use crate::module::CommandError;

/// p.20 "Example Linear Taper Level Values" (also Qu-5/6/7 p.21) as
/// (dB, VC, VF). -inf is 00 00.
const LINEAR_PAIRS: &[(f64, u8, u8)] = &[
    (-89.0, 0x24, 0x16),
    (-85.0, 0x27, 0x71),
    (-80.0, 0x2C, 0x42),
    (-75.0, 0x31, 0x14),
    (-70.0, 0x35, 0x65),
    (-65.0, 0x3A, 0x37),
    (-60.0, 0x3F, 0x09),
    (-55.0, 0x43, 0x5A),
    (-50.0, 0x48, 0x2C),
    (-45.0, 0x4C, 0x7D),
    (-40.0, 0x51, 0x4F),
    (-38.0, 0x53, 0x3C),
    (-36.0, 0x55, 0x2A),
    (-35.0, 0x56, 0x21),
    (-34.0, 0x57, 0x17),
    (-33.0, 0x58, 0x0E),
    (-32.0, 0x59, 0x05),
    (-31.0, 0x59, 0x7C),
    (-30.0, 0x5A, 0x72),
    (-29.0, 0x5B, 0x69),
    (-28.0, 0x5C, 0x60),
    (-27.0, 0x5D, 0x56),
    (-26.0, 0x5E, 0x4D),
    (-25.0, 0x5F, 0x44),
    (-24.0, 0x60, 0x3B),
    (-23.0, 0x61, 0x31),
    (-22.0, 0x62, 0x28),
    (-21.0, 0x63, 0x1F),
    (-20.0, 0x64, 0x16),
    (-19.0, 0x65, 0x0C),
    (-18.0, 0x66, 0x03),
    (-17.0, 0x66, 0x7A),
    (-16.0, 0x67, 0x70),
    (-15.0, 0x68, 0x67),
    (-14.0, 0x69, 0x5E),
    (-13.0, 0x6A, 0x55),
    (-12.0, 0x6B, 0x4B),
    (-11.0, 0x6C, 0x42),
    (-10.0, 0x6D, 0x39),
    (-9.0, 0x6E, 0x2F),
    (-8.0, 0x6F, 0x26),
    (-7.0, 0x70, 0x1D),
    (-6.0, 0x71, 0x14),
    (-5.0, 0x72, 0x0A),
    (-4.0, 0x73, 0x01),
    (-3.0, 0x73, 0x78),
    (-2.0, 0x74, 0x6F),
    (-1.0, 0x75, 0x65),
    (0.0, 0x76, 0x5C),
    (1.0, 0x77, 0x53),
    (2.0, 0x78, 0x49),
    (3.0, 0x79, 0x40),
    (4.0, 0x7A, 0x37),
    (5.0, 0x7B, 0x2E),
    (6.0, 0x7C, 0x24),
    (7.0, 0x7D, 0x1B),
    (8.0, 0x7E, 0x12),
    (9.0, 0x7F, 0x08),
    (10.0, 0x7F, 0x7F),
];

/// p.20 "Approximate Audio Taper Level Values" (also Qu-5/6/7 p.21 and CQ
/// p.15 "Example Level Values"), written here as (VC, VF) pairs.
const AUDIO_PAIRS: &[(f64, u8, u8)] = &[
    (-89.0, 0x01, 0x40),
    (-85.0, 0x02, 0x00),
    (-80.0, 0x02, 0x40),
    (-75.0, 0x03, 0x40),
    (-70.0, 0x04, 0x00),
    (-65.0, 0x05, 0x00),
    (-60.0, 0x06, 0x00),
    (-55.0, 0x07, 0x00),
    (-50.0, 0x08, 0x00),
    (-45.0, 0x0C, 0x00),
    (-40.0, 0x0F, 0x40),
    (-38.0, 0x12, 0x40),
    (-36.0, 0x15, 0x40),
    (-35.0, 0x17, 0x00),
    (-34.0, 0x19, 0x00),
    (-33.0, 0x1A, 0x40),
    (-32.0, 0x1C, 0x00),
    (-31.0, 0x1D, 0x40),
    (-30.0, 0x1F, 0x00),
    (-29.0, 0x20, 0x40),
    (-28.0, 0x22, 0x00),
    (-27.0, 0x23, 0x40),
    (-26.0, 0x25, 0x00),
    (-25.0, 0x26, 0x40),
    (-24.0, 0x28, 0x40),
    (-23.0, 0x2A, 0x00),
    (-22.0, 0x2B, 0x40),
    (-21.0, 0x2D, 0x00),
    (-20.0, 0x2E, 0x40),
    (-19.0, 0x30, 0x00),
    (-18.0, 0x31, 0x40),
    (-17.0, 0x33, 0x00),
    (-16.0, 0x34, 0x40),
    (-15.0, 0x36, 0x00),
    (-14.0, 0x38, 0x00),
    (-13.0, 0x39, 0x40),
    (-12.0, 0x3B, 0x00),
    (-11.0, 0x3C, 0x40),
    (-10.0, 0x3E, 0x00),
    (-9.0, 0x41, 0x40),
    (-8.0, 0x44, 0x40),
    (-7.0, 0x48, 0x00),
    (-6.0, 0x4B, 0x00),
    (-5.0, 0x4E, 0x40),
    (-4.0, 0x52, 0x40),
    (-3.0, 0x56, 0x40),
    (-2.0, 0x5A, 0x00),
    (-1.0, 0x5E, 0x00),
    (0.0, 0x62, 0x00),
    (1.0, 0x65, 0x40),
    (2.0, 0x69, 0x00),
    (3.0, 0x6C, 0x40),
    (4.0, 0x70, 0x00),
    (5.0, 0x73, 0x40),
    (6.0, 0x75, 0x40),
    (7.0, 0x78, 0x00),
    (8.0, 0x7A, 0x40),
    (9.0, 0x7D, 0x00),
    (10.0, 0x7F, 0x40),
];

/// (dB, VC, VF) rows to (dB, 14-bit value).
const fn table(pairs: &[(f64, u8, u8)]) -> [(f64, u16); 59] {
    let mut out = [(0.0, 0u16); 59];
    let mut i = 0;
    while i < 59 {
        let (db, vc, vf) = pairs[i];
        out[i] = (db, ((vc as u16) << 7) | vf as u16);
        i += 1;
    }
    out
}

pub(crate) static LINEAR_TABLE: [(f64, u16); 59] = table(LINEAR_PAIRS);
pub(crate) static AUDIO_TABLE: [(f64, u16); 59] = table(AUDIO_PAIRS);

const PAN_MAX: u16 = 0x3FFF;

/// p.20 "Example Pan/Balance Values" (also Qu-5/6/7 p.21), pan as a
/// percentage from -100 (L100%) to 100 (R100%). The values follow no single
/// formula (L30% is 2C 65 but R50% 5F 7F), so the table is used.
pub(crate) static PAN_TABLE: &[(f64, u16)] = &[
    (-100.0, 0x0000),
    (-90.0, 0x0333),
    (-80.0, 0x0666),
    (-70.0, 0x0999),
    (-60.0, 0x0CCC),
    (-50.0, 0x0FFF),
    (-40.0, 0x1332),
    (-30.0, 0x1665),
    (-20.0, 0x1998),
    (-15.0, 0x1B32),
    (-10.0, 0x1CCB),
    (-5.0, 0x1E65),
    (0.0, 0x1FFF),
    (5.0, 0x2198),
    (10.0, 0x2332),
    (15.0, 0x24CB),
    (20.0, 0x2665),
    (30.0, 0x2998),
    (40.0, 0x2CCB),
    (50.0, 0x2FFF),
    (60.0, 0x3332),
    (70.0, 0x3665),
    (80.0, 0x3998),
    (90.0, 0x3CCB),
    (100.0, 0x3FFF),
];

/// CQ p.15: the same table with centre at 40 00.
pub(crate) static CQ_PAN_TABLE: &[(f64, u16)] = &[
    (-100.0, 0x0000),
    (-90.0, 0x0333),
    (-80.0, 0x0666),
    (-70.0, 0x0999),
    (-60.0, 0x0CCC),
    (-50.0, 0x0FFF),
    (-40.0, 0x1332),
    (-30.0, 0x1665),
    (-20.0, 0x1998),
    (-15.0, 0x1B32),
    (-10.0, 0x1CCB),
    (-5.0, 0x1E65),
    (0.0, 0x2000),
    (5.0, 0x2198),
    (10.0, 0x2332),
    (15.0, 0x24CB),
    (20.0, 0x2665),
    (30.0, 0x2998),
    (40.0, 0x2CCB),
    (50.0, 0x2FFF),
    (60.0, 0x3332),
    (70.0, 0x3665),
    (80.0, 0x3998),
    (90.0, 0x3CCB),
    (100.0, 0x3FFF),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Input,
    StereoInput,
    Usb,
    Bluetooth,
    Group,
    FxReturn,
    Main,
    Aux,
    FxSend,
    Matrix,
    Dca,
    MuteGroup,
}

type Ch = (Kind, u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Param {
    Mute(Ch),
    /// A channel's own level (a source to LR, or an output's master), or a
    /// send to a destination.
    Level(Ch, Option<Ch>),
    Pan(Ch, Option<Ch>),
    Assign(Ch, Ch),
}

const LEVEL_BASE: u16 = 0x2000;
const PAN_BASE: u16 = 0x2800;
const ASSIGN_BASE: u16 = 0x3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Product {
    Sq,
    SqPlus,
    Qu567,
    Cq,
}

/// What each family has, and where.
struct Family {
    product: Product,
    inputs: u8,
    /// Stereo sources: kind, number, source index (the left channel's).
    stereo: &'static [(Kind, u8, u8)],
    groups: u8,
    fx_returns: u8,
    auxes: u8,
    matrices: u8,
    dcas: u8,
    mute_groups: u8,
    /// Output levels start this far after the level base.
    out_base: u16,
    /// Where DCA levels start within the output block.
    dca_out: u16,
    /// Matrix sends per mix.
    mtx_stride: u16,
    scenes: u16,
    /// Channels the mute table leaves out: groups on Qu-5/6/7 (Qu p.22), FX
    /// returns on CQ (CQ p.16).
    unmuted: &'static [Kind],
    /// Qu-5/6/7 lists no group-to-matrix sends (Qu p.25, p.27, p.31).
    group_matrix_sends: bool,
}

const SQ: Family = Family {
    product: Product::Sq,
    inputs: 48,
    stereo: &[],
    groups: 12,
    fx_returns: 8,
    auxes: 12,
    matrices: 3,
    dcas: 8,
    mute_groups: 8,
    out_base: 0x780,
    dca_out: 0x20,
    mtx_stride: 3,
    scenes: 300,
    unmuted: &[],
    group_matrix_sends: true,
};

/// SQ+ article: six matrices, matrix sends 6 per mix, outputs from 4F 50.
const SQ_PLUS: Family = Family {
    product: Product::SqPlus,
    matrices: 6,
    out_base: 0x7D0,
    dca_out: 0x17,
    mtx_stride: 6,
    ..SQ
};

/// Qu-5/6/7 Issue 2 p.22-31.
const QU567: Family = Family {
    product: Product::Qu567,
    inputs: 32,
    stereo: &[
        (Kind::StereoInput, 1, 0x20),
        (Kind::StereoInput, 2, 0x22),
        (Kind::Usb, 1, 0x24),
    ],
    fx_returns: 6,
    unmuted: &[Kind::Group],
    group_matrix_sends: false,
    ..SQ
};

/// CQ V1.2 Issue 5 p.16-18.
const CQ: Family = Family {
    product: Product::Cq,
    inputs: 16,
    stereo: &[
        (Kind::StereoInput, 1, 0x18),
        (Kind::StereoInput, 2, 0x1A),
        (Kind::Usb, 1, 0x1C),
        (Kind::Bluetooth, 1, 0x1E),
    ],
    groups: 0,
    fx_returns: 4,
    auxes: 6,
    matrices: 0,
    dcas: 4,
    mute_groups: 4,
    scenes: 128,
    unmuted: &[Kind::FxReturn],
    ..SQ
};

impl Family {
    fn kind_name(&self, k: Kind) -> &'static str {
        match k {
            Kind::Input => "input",
            Kind::StereoInput => "stereo_input",
            Kind::Usb => "usb",
            Kind::Bluetooth => "bluetooth",
            Kind::Group => "group",
            Kind::FxReturn => "fx_return",
            Kind::Main => "main",
            Kind::Aux if self.product == Product::Cq => "output",
            Kind::Aux => "aux",
            Kind::FxSend => "fx_send",
            Kind::Matrix => "matrix",
            Kind::Dca => "dca",
            Kind::MuteGroup => "mute_group",
        }
    }

    fn kind(&self, name: &str) -> Option<Kind> {
        [
            Kind::Input,
            Kind::StereoInput,
            Kind::Usb,
            Kind::Bluetooth,
            Kind::Group,
            Kind::FxReturn,
            Kind::Main,
            Kind::Aux,
            Kind::FxSend,
            Kind::Matrix,
            Kind::Dca,
            Kind::MuteGroup,
        ]
        .into_iter()
        .find(|k| self.kind_name(*k) == name)
    }

    fn count(&self, k: Kind) -> u8 {
        match k {
            Kind::Input => self.inputs,
            Kind::StereoInput | Kind::Usb | Kind::Bluetooth => {
                self.stereo.iter().filter(|(s, ..)| *s == k).count() as u8
            }
            Kind::Group => self.groups,
            Kind::FxReturn => self.fx_returns,
            Kind::Main => 1,
            Kind::Aux => self.auxes,
            Kind::FxSend => 4,
            Kind::Matrix => self.matrices,
            Kind::Dca => self.dcas,
            Kind::MuteGroup => self.mute_groups,
        }
    }

    fn channels(&self, k: Kind) -> impl Iterator<Item = Ch> {
        (1..=self.count(k)).map(move |n| (k, n))
    }

    /// The source index of an input, stereo source, group or FX return.
    fn source(&self, (k, n): Ch) -> Option<u16> {
        if n == 0 || n > self.count(k) {
            return None;
        }
        let n = n as u16 - 1;
        match k {
            Kind::Input => Some(n),
            Kind::StereoInput | Kind::Usb | Kind::Bluetooth => self
                .stereo
                .iter()
                .filter(|(s, ..)| *s == k)
                .nth(n as usize)
                .map(|(.., i)| *i as u16),
            Kind::Group => Some(0x30 + n),
            Kind::FxReturn => Some(0x3C + n),
            _ => None,
        }
    }

    /// A mix that feeds matrices: LR 0, auxes 1-12, groups 13-24.
    fn mix(&self, (k, n): Ch) -> Option<u16> {
        if self.matrices == 0 || n == 0 || n > self.count(k) {
            return None;
        }
        match k {
            Kind::Main => Some(0),
            Kind::Aux => Some(n as u16),
            Kind::Group if self.group_matrix_sends => Some(12 + n as u16),
            _ => None,
        }
    }

    /// Where the documents leave a group-to-aux cell empty. SQ and SQ+ list
    /// group g to aux a only for a <= 12 - g (p.23, p.26, p.30); Qu-5/6/7
    /// leaves out a group to the aux of the same number (Qu p.24, p.27, p.29).
    fn group_aux_listed(&self, g: u8, a: u8) -> bool {
        match self.product {
            Product::Sq | Product::SqPlus => a as u16 + g as u16 <= 12,
            Product::Qu567 => a != g,
            Product::Cq => false,
        }
    }

    fn checked(&self, (k, n): Ch) -> Option<Ch> {
        (n >= 1 && n <= self.count(k)).then_some((k, n))
    }

    fn address(&self, p: Param) -> Option<u16> {
        match p {
            Param::Mute(ch) => {
                let (k, n) = self.checked(ch)?;
                let n = n as u16;
                Some(match k {
                    Kind::Main => 0x44,
                    Kind::Aux => 0x44 + n,
                    Kind::FxSend => 0x50 + n,
                    Kind::Matrix => 0x54 + n,
                    Kind::Dca => 0x100 + n - 1,
                    Kind::MuteGroup => 0x200 + n - 1,
                    k if self.unmuted.contains(&k) => return None,
                    _ => self.source(ch)?,
                })
            }
            Param::Level(ch, None) => self
                .source(ch)
                .map(|s| LEVEL_BASE + s)
                .or_else(|| self.output(ch).map(|o| LEVEL_BASE + self.out_base + o)),
            Param::Pan(ch, None) => match self.source(ch) {
                Some(s) => Some(PAN_BASE + s),
                None => self.balance(ch).map(|o| PAN_BASE + self.out_base + o),
            },
            Param::Level(src, Some(dst)) => self.send(LEVEL_BASE, src, dst, false),
            Param::Pan(src, Some(dst)) => self.send(PAN_BASE, src, dst, true),
            Param::Assign(src, dst) => {
                if self.product == Product::Cq {
                    return None;
                }
                let dst = self.checked(dst)?;
                let s = self.source(src);
                match (src.0, dst.0) {
                    (_, Kind::Main) if s.is_some() => Some(ASSIGN_BASE + s?),
                    (Kind::Input, Kind::Group)
                        if self.product == Product::Sq || self.product == Product::SqPlus =>
                    {
                        Some(ASSIGN_BASE + 0x374 + s? * 12 + dst.1 as u16 - 1)
                    }
                    (Kind::FxReturn, Kind::Group) if self.groups > 0 => {
                        Some(ASSIGN_BASE + 0x5B4 + (src.1 as u16 - 1) * 12 + dst.1 as u16 - 1)
                    }
                    (_, Kind::Group) => None,
                    _ => self.send(ASSIGN_BASE, src, dst, false),
                }
            }
        }
    }

    /// An output's position in the output block (levels).
    fn output(&self, ch: Ch) -> Option<u16> {
        let (k, n) = self.checked(ch)?;
        let n = n as u16;
        match k {
            Kind::Main => Some(0),
            Kind::Aux => Some(n),
            Kind::FxSend => Some(12 + n),
            Kind::Matrix => Some(16 + n),
            Kind::Dca => Some(self.dca_out + n - 1),
            _ => None,
        }
    }

    /// Output balance: SQ and SQ+ list LR, auxes and matrices 1-3 (p.27, SQ+
    /// "Balance Parameter Numbers – Mix Sends"); Qu-5/6/7 and CQ list none.
    fn balance(&self, ch: Ch) -> Option<u16> {
        if !matches!(self.product, Product::Sq | Product::SqPlus) {
            return None;
        }
        match ch {
            (Kind::Main | Kind::Aux, _) => self.output(ch),
            (Kind::Matrix, m) if m <= 3 => self.output(ch),
            _ => None,
        }
    }

    fn send(&self, base: u16, src: Ch, dst: Ch, pan: bool) -> Option<u16> {
        let (dk, dn) = self.checked(dst)?;
        let d = dn as u16;
        match dk {
            Kind::Aux => {
                let s = self.source(src)?;
                if src.0 == Kind::Group && !self.group_aux_listed(src.1, dn) {
                    return None;
                }
                if pan {
                    let listed = match self.product {
                        // Qu p.26: pan to Aux1&2, 3&4, 5&6 by the odd one, and 7-12.
                        Product::Qu567 => dn >= 7 || dn % 2 == 1,
                        // CQ p.18: Out1/2, Out3/4, Out5/6.
                        Product::Cq => dn % 2 == 1,
                        _ => true,
                    };
                    if !listed {
                        return None;
                    }
                }
                Some(base + 0x44 + s * 12 + d - 1)
            }
            Kind::FxSend if !pan => {
                let s = self.source(src)?;
                // CQ p.17: an FX return has no send to its own FX unit.
                if self.product == Product::Cq && src.0 == Kind::FxReturn && src.1 == dn {
                    return None;
                }
                Some(base + 0x614 + s * 4 + d - 1)
            }
            Kind::Matrix => {
                let mix = self.mix(src)?;
                // Qu p.27: matrix pan by the odd matrix of a pair.
                if pan && self.product == Product::Qu567 && dn % 2 == 0 {
                    return None;
                }
                Some(base + 0x724 + mix * self.mtx_stride + d - 1)
            }
            _ => None,
        }
    }

    fn sources(&self) -> Vec<Ch> {
        let mut v: Vec<Ch> = self.channels(Kind::Input).collect();
        for (k, n, _) in self.stereo {
            v.push((*k, *n));
        }
        v.extend(self.channels(Kind::Group));
        v.extend(self.channels(Kind::FxReturn));
        v
    }

    fn outputs(&self) -> Vec<Ch> {
        let mut v = vec![(Kind::Main, 1)];
        for k in [Kind::Aux, Kind::FxSend, Kind::Matrix, Kind::Dca] {
            v.extend(self.channels(k));
        }
        v
    }

    /// Every parameter the family documents, own values first.
    fn params(&self) -> (Vec<Param>, Vec<Param>) {
        let sources = self.sources();
        let outputs = self.outputs();
        let mut own = Vec::new();
        let mut sends = Vec::new();
        for ch in sources.iter().chain(&outputs) {
            own.push(Param::Mute(*ch));
            own.push(Param::Level(*ch, None));
            own.push(Param::Pan(*ch, None));
        }
        for ch in self.channels(Kind::MuteGroup) {
            own.push(Param::Mute(ch));
        }
        for src in &sources {
            own.push(Param::Assign(*src, (Kind::Main, 1)));
        }
        let mut dests: Vec<Ch> = self.channels(Kind::Aux).collect();
        dests.extend(self.channels(Kind::FxSend));
        dests.extend(self.channels(Kind::Group));
        dests.extend(self.channels(Kind::Matrix));
        let mixes: Vec<Ch> = std::iter::once((Kind::Main, 1))
            .chain(self.channels(Kind::Aux))
            .chain(self.channels(Kind::Group))
            .collect();
        for src in sources.iter().chain(&mixes) {
            for dst in &dests {
                sends.push(Param::Level(*src, Some(*dst)));
                sends.push(Param::Pan(*src, Some(*dst)));
                sends.push(Param::Assign(*src, *dst));
            }
        }
        own.retain(|p| self.address(*p).is_some());
        sends.retain(|p| self.address(*p).is_some());
        (own, sends)
    }
}

pub(crate) struct Sq {
    family: &'static Family,
    n: u8,
    law: Law,
    pan: Law,
    soft_keys: u8,
    sync_sends: bool,
    by_address: HashMap<u16, Param>,
    own: Vec<Param>,
    sends: Vec<Param>,
}

impl Sq {
    pub(crate) fn new(model: &str, settings: &Params) -> Result<Sq, String> {
        let (family, soft_keys): (&'static Family, u8) = match model {
            "sq-5" => (&SQ, 8),
            "sq-6" | "sq-7" => (&SQ, 16),
            "sq-5-plus" => (&SQ_PLUS, 8),
            "sq-6-plus" | "sq-7-plus" => (&SQ_PLUS, 16),
            "qu-5" | "qu-6" | "qu-7" => (&QU567, 16),
            "cq-12t" | "cq-18t" | "cq-20b" => (&CQ, 3),
            other => return Err(format!("unknown SQ/Qu/CQ model '{other}'")),
        };
        let cq = family.product == Product::Cq;
        // CQ "uses MIDI Channel 1 for all control messaging" (CQ p.4).
        let n = if cq { 0 } else { midi_channel(settings, 16)? };
        let audio = cq || settings.get("fader_law").and_then(Value::as_str) == Some("audio");
        let law = if audio {
            Law::Table(&AUDIO_TABLE)
        } else {
            Law::Table(&LINEAR_TABLE)
        };
        let (own, sends) = family.params();
        let by_address = own
            .iter()
            .chain(&sends)
            .filter_map(|p| family.address(*p).map(|a| (a, *p)))
            .collect();
        let pan = Law::Table(if cq { CQ_PAN_TABLE } else { PAN_TABLE });
        Ok(Sq {
            family,
            n,
            law,
            pan,
            soft_keys,
            sync_sends: setting_flag(settings, "sync_sends", false),
            by_address,
            own,
            sends,
        })
    }

    fn arg_ch(&self, a: &Args, kind_key: &str, num_key: &str) -> Result<Ch, CommandError> {
        let name = a.str(kind_key)?;
        let k = self
            .family
            .kind(name)
            .filter(|k| self.family.count(*k) > 0)
            .ok_or_else(|| invalid(format!("this console has no '{name}' channels")))?;
        let n = a.int(num_key)?;
        let count = self.family.count(k) as i64;
        if n < 1 || n > count {
            return Err(invalid(format!("{name} {n} is outside 1 to {count}")));
        }
        Ok((k, n as u8))
    }

    fn channel(&self, a: &Args) -> Result<Ch, CommandError> {
        self.arg_ch(a, "channel_type", "channel")
    }

    fn destination(&self, a: &Args) -> Result<Ch, CommandError> {
        self.arg_ch(a, "destination_type", "destination")
    }

    fn address(&self, p: Param) -> Result<(u8, u8), CommandError> {
        self.family
            .address(p)
            .map(|a| ((a >> 7) as u8, (a & 0x7F) as u8))
            .ok_or_else(|| invalid("the console's protocol has no such parameter"))
    }

    fn get(&self, (mb, lb): (u8, u8)) -> Vec<u8> {
        midi::nrpn_step(self.n, mb, lb, 0x60, 0x7F)
    }

    fn set(&self, p: Param, coarse: u8, fine: u8) -> Result<Plan, CommandError> {
        let at = self.address(p)?;
        Ok(Plan::write_then(
            midi::nrpn_fine(self.n, at.0, at.1, coarse, fine),
            self.get(at),
        ))
    }

    fn step(&self, p: Param, up: bool) -> Result<Plan, CommandError> {
        let at = self.address(p)?;
        Ok(Plan::write_then(
            midi::nrpn_step(self.n, at.0, at.1, if up { 0x60 } else { 0x61 }, 0x00),
            self.get(at),
        ))
    }

    fn read(&self, p: Param) -> Result<Plan, CommandError> {
        let at = self.address(p)?;
        let path = self.path(p);
        let send = self.get(at);
        Ok(match p {
            Param::Level(..) => Plan::read_fields(
                send,
                format!("{path}.level_raw"),
                &["level_db", "level_raw"],
            ),
            Param::Pan(..) => {
                Plan::read_fields(send, format!("{path}.pan_raw"), &["pan", "pan_raw"])
            }
            Param::Mute(_) => Plan::read(send, format!("{path}.mute")),
            Param::Assign(_, (Kind::Main, _)) => Plan::read(send, format!("{path}.main_assign")),
            Param::Assign(..) => Plan::read(send, format!("{path}.assigned")),
        })
    }

    fn ch_path(&self, (k, n): Ch) -> String {
        format!("channels.{}.{n}", self.family.kind_name(k))
    }

    /// The state object a parameter lives in.
    fn path(&self, p: Param) -> String {
        match p {
            Param::Mute(c)
            | Param::Level(c, None)
            | Param::Pan(c, None)
            | Param::Assign(c, (Kind::Main, _)) => self.ch_path(c),
            Param::Level(s, Some(d)) | Param::Pan(s, Some(d)) | Param::Assign(s, d) => format!(
                "sends.{}.{}.{}.{}",
                self.family.kind_name(s.0),
                s.1,
                self.family.kind_name(d.0),
                d.1
            ),
        }
    }

    fn pan_arg(&self, a: &Args) -> Result<u16, CommandError> {
        a.value("pan", "pan_raw", &self.pan, PAN_MAX)
    }

    fn updates(&self, p: Param, coarse: Option<u8>, fine: u8) -> Vec<Update> {
        let path = self.path(p);
        let value = midi::join14(coarse.unwrap_or(0), fine);
        match p {
            Param::Mute(_) => vec![(format!("{path}.mute"), Value::Bool(fine != 0))],
            Param::Assign(_, (Kind::Main, _)) => {
                vec![(format!("{path}.main_assign"), Value::Bool(fine != 0))]
            }
            Param::Assign(..) => vec![(format!("{path}.assigned"), Value::Bool(fine != 0))],
            Param::Level(..) => level_updates(&path, "level", &self.law, value),
            Param::Pan(..) => level_updates(&path, "pan", &self.pan, value)
                .into_iter()
                .map(|(k, v)| (k.replace("pan_db", "pan"), v))
                .collect(),
        }
    }
}

impl Dialect for Sq {
    fn command(&mut self, name: &str, a: &Args) -> Result<Plan, CommandError> {
        let n = self.n;
        let send_param = |a: &Args| -> Result<(Ch, Ch), CommandError> {
            Ok((self.channel(a)?, self.destination(a)?))
        };
        let up = |a: &Args| -> Result<bool, CommandError> {
            Ok(matches!(a.str("direction")?, "up" | "right"))
        };
        match name {
            "set_mute" => {
                let v = u8::from(a.flag("muted")?);
                self.set(Param::Mute(self.channel(a)?), 0, v)
            }
            "toggle_mute" => {
                let ch = self.channel(a)?;
                // CQ p.8: "Mute Toggle cannot currently be used with DCA Mute
                // and Mute Group Mute."
                if self.family.product == Product::Cq && matches!(ch.0, Kind::Dca | Kind::MuteGroup)
                {
                    return Err(invalid("CQ cannot toggle DCA or mute group mutes"));
                }
                self.step(Param::Mute(ch), true)
            }
            "get_mute" => self.read(Param::Mute(self.channel(a)?)),
            "set_level" => {
                let p = Param::Level(self.channel(a)?, None);
                let (vc, vf) = midi::split14(a.level("level", &self.law, PAN_MAX)?);
                self.set(p, vc, vf)
            }
            "adjust_level" => self.step(Param::Level(self.channel(a)?, None), up(a)?),
            "get_level" => self.read(Param::Level(self.channel(a)?, None)),
            "set_send_level" => {
                let (s, d) = send_param(a)?;
                let (vc, vf) = midi::split14(a.level("level", &self.law, PAN_MAX)?);
                self.set(Param::Level(s, Some(d)), vc, vf)
            }
            "adjust_send_level" => {
                let (s, d) = send_param(a)?;
                self.step(Param::Level(s, Some(d)), up(a)?)
            }
            "get_send_level" => {
                let (s, d) = send_param(a)?;
                self.read(Param::Level(s, Some(d)))
            }
            "set_pan" => {
                let p = Param::Pan(self.channel(a)?, None);
                let (vc, vf) = midi::split14(self.pan_arg(a)?);
                self.set(p, vc, vf)
            }
            "adjust_pan" => self.step(Param::Pan(self.channel(a)?, None), up(a)?),
            "get_pan" => self.read(Param::Pan(self.channel(a)?, None)),
            "set_send_pan" => {
                let (s, d) = send_param(a)?;
                let (vc, vf) = midi::split14(self.pan_arg(a)?);
                self.set(Param::Pan(s, Some(d)), vc, vf)
            }
            "adjust_send_pan" => {
                let (s, d) = send_param(a)?;
                self.step(Param::Pan(s, Some(d)), up(a)?)
            }
            "get_send_pan" => {
                let (s, d) = send_param(a)?;
                self.read(Param::Pan(s, Some(d)))
            }
            "set_main_assign" => {
                let v = u8::from(a.flag("assigned")?);
                self.set(Param::Assign(self.channel(a)?, (Kind::Main, 1)), 0, v)
            }
            "toggle_main_assign" => {
                self.step(Param::Assign(self.channel(a)?, (Kind::Main, 1)), true)
            }
            "get_main_assign" => self.read(Param::Assign(self.channel(a)?, (Kind::Main, 1))),
            "set_send_assign" => {
                let (s, d) = send_param(a)?;
                let v = u8::from(a.flag("assigned")?);
                self.set(Param::Assign(s, d), 0, v)
            }
            "toggle_send_assign" => {
                let (s, d) = send_param(a)?;
                self.step(Param::Assign(s, d), true)
            }
            "get_send_assign" => {
                let (s, d) = send_param(a)?;
                self.read(Param::Assign(s, d))
            }
            "recall_scene" | "recall_cue" => {
                let key = if name == "recall_cue" { "cue" } else { "scene" };
                let s = a.int(key)?;
                let max = self.family.scenes as i64;
                if s < 1 || s > max {
                    return Err(invalid(format!("{key} is 1-{max}")));
                }
                let i = (s - 1) as u16;
                // SQ p.9: cue list positions use banks 0A-0C.
                let bank = (i / 128) as u8 + if name == "recall_cue" { 0x0A } else { 0 };
                Ok(Plan::write(midi::bank_program(n, bank, (i % 128) as u8)))
            }
            "soft_key" => {
                let k = a.int("key")?;
                if k < 1 || k > self.soft_keys as i64 {
                    return Err(invalid(format!("key is 1-{}", self.soft_keys)));
                }
                let note = 0x30 + (k - 1) as u8;
                Ok(Plan::write(if a.flag("pressed")? {
                    midi::note_on(n, note, 0x7F)
                } else {
                    midi::note_off(n, note, 0x00)
                }))
            }
            "set_ufx_global_key" | "set_ufx_global_scale" => {
                let (key, list, cc): (&str, &[&str], u8) = if name == "set_ufx_global_key" {
                    (
                        "key",
                        &[
                            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                        ],
                        0x0C,
                    )
                } else {
                    ("scale", &["major", "minor", "chromatic"], 0x0D)
                };
                let v = a.str(key)?;
                let i = list
                    .iter()
                    .position(|x| *x == v)
                    .ok_or_else(|| invalid(format!("unknown {key} '{v}'")))?;
                Ok(Plan::write(midi::cc(n, cc, i as u8)))
            }
            other => Err(unknown(other)),
        }
    }

    fn event(&mut self, event: &Event) -> Vec<Update> {
        match *event {
            Event::NrpnFine {
                ch,
                msb,
                lsb,
                coarse,
                fine,
            } if ch == self.n => {
                let at = ((msb as u16) << 7) | lsb as u16;
                match self.by_address.get(&at) {
                    Some(p) => self.updates(*p, coarse, fine),
                    None => Vec::new(),
                }
            }
            Event::Program {
                ch,
                bank_msb,
                program,
                ..
            } if ch == self.n => {
                let bank = bank_msb.unwrap_or(0);
                let (key, base) = match bank {
                    0x00..=0x02 => ("scene.current", bank),
                    0x0A..=0x0C => ("cue.current", bank - 0x0A),
                    _ => return Vec::new(),
                };
                vec![(key.into(), json!(base as u32 * 128 + program as u32 + 1))]
            }
            _ => Vec::new(),
        }
    }

    fn sync(&self) -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = self
            .own
            .iter()
            .filter_map(|p| self.address(*p).ok().map(|at| self.get(at)))
            .collect();
        if self.sync_sends {
            out.extend(
                self.sends
                    .iter()
                    .filter_map(|p| self.address(*p).ok().map(|at| self.get(at))),
            );
        }
        out
    }

    fn probe(&self) -> Vec<u8> {
        // The LR mute (00 44).
        self.get((0x00, 0x44))
    }

    fn silence_hint(&self) -> &'static str {
        "check that the midi_channel setting matches the console (Utility / General / MIDI), \
         and on CQ and Qu that no other app holds the one TCP MIDI connection"
    }
}

#[cfg(test)]
mod tests {
    use super::super::allenheath::testing::*;
    use super::*;
    use crate::module::{Action, Outcome};

    fn sq(model: &str, extra: Value) -> Box<Sq> {
        let mut p = json!({"midi_channel": 1});
        if let (Some(p), Some(e)) = (p.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        Box::new(Sq::new(model, &params(p)).unwrap())
    }

    fn addr(f: &Family, p: Param) -> Option<(u8, u8)> {
        f.address(p).map(|a| ((a >> 7) as u8, (a & 0x7F) as u8))
    }

    const IP: Kind = Kind::Input;

    #[test]
    fn audio_table_is_the_documents_pairs() {
        assert_eq!(AUDIO_TABLE[0], (-89.0, (0x01 << 7) | 0x40));
        assert_eq!(AUDIO_TABLE[48], (0.0, 0x62 << 7));
        assert_eq!(LINEAR_TABLE[48], (0.0, (0x76 << 7) | 0x5C));
        for law in [Law::Table(&LINEAR_TABLE), Law::Table(&AUDIO_TABLE)] {
            if let Law::Table(t) = law {
                for &(db, raw) in t {
                    assert_eq!(law.encode(db), Some(raw), "{db}");
                    assert_eq!(law.decode(raw), Some(db), "{raw:04X}");
                }
            }
        }
    }

    #[test]
    fn parameter_numbers_match_the_sq_tables() {
        let f = &SQ;
        let lvl = |s, d| addr(f, Param::Level(s, d));
        // p.21 mutes.
        assert_eq!(addr(f, Param::Mute((IP, 48))), Some((0x00, 0x2F)));
        assert_eq!(addr(f, Param::Mute((Kind::Main, 1))), Some((0x00, 0x44)));
        assert_eq!(addr(f, Param::Mute((Kind::Aux, 12))), Some((0x00, 0x50)));
        assert_eq!(addr(f, Param::Mute((Kind::FxSend, 4))), Some((0x00, 0x54)));
        assert_eq!(addr(f, Param::Mute((Kind::Matrix, 3))), Some((0x00, 0x57)));
        assert_eq!(addr(f, Param::Mute((Kind::Dca, 8))), Some((0x02, 0x07)));
        assert_eq!(
            addr(f, Param::Mute((Kind::MuteGroup, 4))),
            Some((0x04, 0x03))
        );
        assert_eq!(
            addr(f, Param::Mute((Kind::FxReturn, 8))),
            Some((0x00, 0x43))
        );
        // p.22-24 levels.
        assert_eq!(lvl((IP, 48), Some((Kind::Aux, 12))), Some((0x45, 0x03)));
        assert_eq!(
            lvl((Kind::Group, 1), Some((Kind::Aux, 11))),
            Some((0x45, 0x0E))
        );
        assert_eq!(
            lvl((Kind::Group, 1), Some((Kind::Aux, 12))),
            None,
            "not listed"
        );
        assert_eq!(
            lvl((Kind::Group, 11), Some((Kind::Aux, 1))),
            Some((0x45, 0x7C))
        );
        assert_eq!(
            lvl((Kind::FxReturn, 8), Some((Kind::Aux, 12))),
            Some((0x46, 0x73))
        );
        assert_eq!(lvl((IP, 25), Some((Kind::FxSend, 1))), Some((0x4C, 0x74)));
        assert_eq!(
            lvl((Kind::Group, 12), Some((Kind::FxSend, 4))),
            Some((0x4E, 0x03))
        );
        assert_eq!(
            lvl((Kind::FxReturn, 8), Some((Kind::FxSend, 4))),
            Some((0x4E, 0x23))
        );
        assert_eq!(
            lvl((Kind::Main, 1), Some((Kind::Matrix, 3))),
            Some((0x4E, 0x26))
        );
        assert_eq!(
            lvl((Kind::Aux, 12), Some((Kind::Matrix, 1))),
            Some((0x4E, 0x48))
        );
        assert_eq!(
            lvl((Kind::Group, 12), Some((Kind::Matrix, 3))),
            Some((0x4E, 0x6E))
        );
        assert_eq!(lvl((Kind::FxSend, 4), None), Some((0x4F, 0x10)));
        assert_eq!(lvl((Kind::Matrix, 1), None), Some((0x4F, 0x11)));
        assert_eq!(lvl((Kind::Dca, 8), None), Some((0x4F, 0x27)));
        // p.25-27 pan and balance.
        assert_eq!(
            addr(f, Param::Pan((IP, 48), Some((Kind::Aux, 12)))),
            Some((0x55, 0x03))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Aux, 12), None)),
            Some((0x5F, 0x0C))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Matrix, 3), None)),
            Some((0x5F, 0x13))
        );
        assert_eq!(addr(f, Param::Pan((Kind::FxSend, 1), None)), None);
        // p.28-31 assignments.
        assert_eq!(
            addr(f, Param::Assign((IP, 1), (Kind::Group, 1))),
            Some((0x66, 0x74))
        );
        assert_eq!(
            addr(f, Param::Assign((IP, 48), (Kind::Group, 12))),
            Some((0x6B, 0x33))
        );
        assert_eq!(
            addr(f, Param::Assign((Kind::FxReturn, 8), (Kind::Group, 12))),
            Some((0x6C, 0x13))
        );
        assert_eq!(
            addr(f, Param::Assign((Kind::FxReturn, 1), (Kind::Aux, 1))),
            Some((0x66, 0x14))
        );
        assert_eq!(
            addr(f, Param::Assign((IP, 1), (Kind::FxSend, 1))),
            Some((0x6C, 0x14))
        );
        assert_eq!(
            addr(f, Param::Assign((Kind::Group, 12), (Kind::Matrix, 3))),
            Some((0x6E, 0x6E))
        );
    }

    #[test]
    fn sq_plus_qu567_and_cq_tables() {
        // SQ+ article: six matrices and the moved output block.
        let f = &SQ_PLUS;
        assert_eq!(
            addr(f, Param::Level((Kind::Aux, 1), Some((Kind::Matrix, 1)))),
            Some((0x4E, 0x2A))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Group, 12), Some((Kind::Matrix, 6)))),
            Some((0x4F, 0x39))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Main, 1), None)),
            Some((0x4F, 0x50))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::FxSend, 4), None)),
            Some((0x4F, 0x60))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Matrix, 6), None)),
            Some((0x4F, 0x66))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Dca, 1), None)),
            Some((0x4F, 0x67))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Dca, 8), None)),
            Some((0x4F, 0x6E))
        );
        assert_eq!(addr(f, Param::Mute((Kind::Matrix, 6))), Some((0x00, 0x5A)));
        assert_eq!(
            addr(f, Param::Pan((Kind::Matrix, 3), None)),
            Some((0x5F, 0x63))
        );
        assert_eq!(addr(f, Param::Pan((Kind::Matrix, 4), None)), None);
        assert_eq!(
            addr(f, Param::Assign((Kind::Group, 12), (Kind::Matrix, 6))),
            Some((0x6F, 0x39))
        );

        // Qu-5/6/7 Issue 2.
        let f = &QU567;
        assert_eq!(addr(f, Param::Mute((Kind::Usb, 1))), Some((0x00, 0x24)));
        assert_eq!(
            addr(f, Param::Level((Kind::Usb, 1), Some((Kind::Aux, 12)))),
            Some((0x43, 0x7F))
        );
        assert_eq!(
            addr(
                f,
                Param::Level((Kind::StereoInput, 2), Some((Kind::FxSend, 1)))
            ),
            Some((0x4D, 0x1C))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Group, 2), Some((Kind::Aux, 2)))),
            None
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Group, 12), Some((Kind::Aux, 11)))),
            Some((0x46, 0x12))
        );
        assert_eq!(
            addr(
                f,
                Param::Level((Kind::FxReturn, 6), Some((Kind::FxSend, 4)))
            ),
            Some((0x4E, 0x1B))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Usb, 1), Some((Kind::Aux, 5)))),
            Some((0x53, 0x78))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Usb, 1), Some((Kind::Aux, 6)))),
            None
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Group, 11), Some((Kind::Aux, 5)))),
            Some((0x56, 0x00))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Main, 1), Some((Kind::Matrix, 3)))),
            Some((0x5E, 0x26))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::Main, 1), Some((Kind::Matrix, 2)))),
            None
        );
        assert_eq!(
            addr(f, Param::Assign((IP, 1), (Kind::Group, 1))),
            None,
            "not documented"
        );
        assert_eq!(
            addr(f, Param::Assign((Kind::FxReturn, 6), (Kind::Group, 12))),
            Some((0x6B, 0x7B))
        );
        assert_eq!(
            addr(f, Param::Assign((Kind::Usb, 1), (Kind::FxSend, 4))),
            Some((0x6D, 0x27))
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Dca, 8), None)),
            Some((0x4F, 0x27))
        );

        // CQ V1.2 Issue 5.
        let f = &CQ;
        assert_eq!(
            addr(f, Param::Mute((Kind::Bluetooth, 1))),
            Some((0x00, 0x1E))
        );
        assert_eq!(addr(f, Param::Mute((Kind::Aux, 6))), Some((0x00, 0x4A)));
        assert_eq!(
            addr(
                f,
                Param::Level((Kind::StereoInput, 2), Some((Kind::Aux, 6)))
            ),
            Some((0x43, 0x01))
        );
        assert_eq!(
            addr(
                f,
                Param::Level((Kind::Bluetooth, 1), Some((Kind::FxSend, 4)))
            ),
            Some((0x4D, 0x0F))
        );
        assert_eq!(
            addr(
                f,
                Param::Level((Kind::FxReturn, 2), Some((Kind::FxSend, 2)))
            ),
            None
        );
        assert_eq!(
            addr(f, Param::Level((Kind::Dca, 4), None)),
            Some((0x4F, 0x23))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::StereoInput, 2), Some((Kind::Aux, 5)))),
            Some((0x53, 0x00))
        );
        assert_eq!(
            addr(f, Param::Pan((Kind::FxReturn, 4), Some((Kind::Aux, 5)))),
            Some((0x56, 0x3C))
        );
        assert_eq!(addr(f, Param::Pan((IP, 1), Some((Kind::Aux, 2)))), None);
        assert_eq!(
            addr(f, Param::Assign((IP, 1), (Kind::Main, 1))),
            None,
            "CQ documents no assignments"
        );
    }

    #[test]
    fn documented_examples_byte_for_byte() {
        // SQ p.11-18, on the channels the examples use.
        let cases: &[(i64, &str, Value, &str)] = &[
            (
                1,
                "set_mute",
                json!({"channel_type": "input", "channel": 1, "muted": true}),
                "B0 63 00 B0 62 00 B0 06 00 B0 26 01",
            ),
            (
                1,
                "set_mute",
                json!({"channel_type": "main", "channel": 1, "muted": false}),
                "B0 63 00 B0 62 44 B0 06 00 B0 26 00",
            ),
            (
                7,
                "set_mute",
                json!({"channel_type": "mute_group", "channel": 4, "muted": true}),
                "B6 63 04 B6 62 03 B6 06 00 B6 26 01",
            ),
            (
                1,
                "toggle_mute",
                json!({"channel_type": "input", "channel": 1}),
                "B0 63 00 B0 62 00 B0 60 00",
            ),
            (
                1,
                "set_level",
                json!({"channel_type": "input", "channel": 1, "level_db": 0.0}),
                "B0 63 40 B0 62 00 B0 06 76 B0 26 5C",
            ),
            (
                1,
                "adjust_level",
                json!({"channel_type": "input", "channel": 1, "direction": "up"}),
                "B0 63 40 B0 62 00 B0 60 00",
            ),
            (
                5,
                "adjust_level",
                json!({"channel_type": "group", "channel": 5, "direction": "down"}),
                "B4 63 40 B4 62 34 B4 61 00",
            ),
            (
                12,
                "adjust_send_level",
                json!({"channel_type": "fx_return", "channel": 2, "destination_type": "aux", "destination": 3, "direction": "up"}),
                "BB 63 46 BB 62 22 BB 60 00",
            ),
            (
                1,
                "set_pan",
                json!({"channel_type": "input", "channel": 1, "pan": -100.0}),
                "B0 63 50 B0 62 00 B0 06 00 B0 26 00",
            ),
            (
                1,
                "set_pan",
                json!({"channel_type": "input", "channel": 1, "pan": 0.0}),
                "B0 63 50 B0 62 00 B0 06 3F B0 26 7F",
            ),
            (
                1,
                "set_pan",
                json!({"channel_type": "input", "channel": 24, "pan": 20.0}),
                "B0 63 50 B0 62 17 B0 06 4C B0 26 65",
            ),
            (
                1,
                "set_send_pan",
                json!({"channel_type": "input", "channel": 24, "destination_type": "aux", "destination": 5, "pan": 20.0}),
                "B0 63 52 B0 62 5C B0 06 4C B0 26 65",
            ),
            (
                4,
                "set_send_pan",
                json!({"channel_type": "input", "channel": 24, "destination_type": "aux", "destination": 5, "pan": -50.0}),
                "B3 63 52 B3 62 5C B3 06 1F B3 26 7F",
            ),
            (
                4,
                "set_send_pan",
                json!({"channel_type": "group", "channel": 3, "destination_type": "aux", "destination": 2, "pan": -50.0}),
                "B3 63 55 B3 62 1D B3 06 1F B3 26 7F",
            ),
            (
                11,
                "set_send_pan",
                json!({"channel_type": "main", "channel": 1, "destination_type": "matrix", "destination": 3, "pan": 100.0}),
                "BA 63 5E BA 62 26 BA 06 7F BA 26 7F",
            ),
            (
                1,
                "adjust_send_pan",
                json!({"channel_type": "input", "channel": 37, "destination_type": "aux", "destination": 8, "direction": "right"}),
                "B0 63 53 B0 62 7B B0 60 00",
            ),
            (
                3,
                "adjust_send_pan",
                json!({"channel_type": "aux", "channel": 5, "destination_type": "matrix", "destination": 1, "direction": "right"}),
                "B2 63 5E B2 62 33 B2 60 00",
            ),
            (
                1,
                "set_main_assign",
                json!({"channel_type": "input", "channel": 1, "assigned": true}),
                "B0 63 60 B0 62 00 B0 06 00 B0 26 01",
            ),
            (
                1,
                "set_send_assign",
                json!({"channel_type": "fx_return", "channel": 1, "destination_type": "aux", "destination": 7, "assigned": true}),
                "B0 63 66 B0 62 1A B0 06 00 B0 26 01",
            ),
            (
                2,
                "set_send_assign",
                json!({"channel_type": "group", "channel": 1, "destination_type": "aux", "destination": 3, "assigned": false}),
                "B1 63 65 B1 62 06 B1 06 00 B1 26 00",
            ),
            (
                4,
                "toggle_send_assign",
                json!({"channel_type": "group", "channel": 2, "destination_type": "matrix", "destination": 2}),
                "B3 63 6E B3 62 4F B3 60 00",
            ),
            (
                1,
                "get_level",
                json!({"channel_type": "input", "channel": 1}),
                "B0 63 40 B0 62 00 B0 60 7F",
            ),
            (
                1,
                "get_send_pan",
                json!({"channel_type": "input", "channel": 30, "destination_type": "aux", "destination": 5}),
                "B0 63 53 B0 62 24 B0 60 7F",
            ),
            (
                5,
                "get_send_pan",
                json!({"channel_type": "aux", "channel": 7, "destination_type": "matrix", "destination": 1}),
                "B4 63 5E B4 62 39 B4 60 7F",
            ),
            (
                12,
                "get_send_assign",
                json!({"channel_type": "fx_return", "channel": 2, "destination_type": "fx_send", "destination": 3}),
                "BB 63 6E BB 62 0A BB 60 7F",
            ),
            (1, "recall_scene", json!({"scene": 7}), "B0 00 00 C0 06"),
            (1, "recall_scene", json!({"scene": 120}), "B0 00 00 C0 77"),
            (3, "recall_scene", json!({"scene": 156}), "B2 00 01 C2 1B"),
            (1, "recall_cue", json!({"cue": 156}), "B0 00 0B C0 1B"),
            (
                1,
                "soft_key",
                json!({"key": 1, "pressed": true}),
                "90 30 7F",
            ),
            (
                5,
                "soft_key",
                json!({"key": 7, "pressed": false}),
                "84 36 00",
            ),
        ];
        for (channel, name, p, want) in cases {
            let (mut m, _) = connected(sq("sq-7", json!({"midi_channel": channel})), &[0xFE]);
            let got = bytes(&mut m, name, p.clone()).unwrap();
            let want = hex(want);
            assert_eq!(&got[..want.len()], &want[..], "{name} {p}");
        }
    }

    #[test]
    fn audio_taper_examples_and_cq() {
        let (mut m, _) = connected(
            sq("sq-6", json!({"fader_law": "audio", "midi_channel": 4})),
            &[0xFE],
        );
        // SQ p.13: Ip40 to Aux5 -12 dB, Grp4 to Aux8 -24 dB, audio taper.
        let got = bytes(
            &mut m,
            "set_send_level",
            json!({"channel_type": "input", "channel": 40,
            "destination_type": "aux", "destination": 5, "level_db": -12.0}),
        )
        .unwrap();
        assert_eq!(got[..12], hex("B3 63 44 B3 62 1C B3 06 3B B3 26 00"));
        let got = bytes(
            &mut m,
            "set_send_level",
            json!({"channel_type": "group", "channel": 4,
            "destination_type": "aux", "destination": 8, "level_db": -24.0}),
        )
        .unwrap();
        assert_eq!(got[..12], hex("B3 63 45 B3 62 2F B3 06 28 B3 26 40"));

        // CQ p.9-13 examples, channel 1 always.
        let (mut m, _) = connected(sq("cq-18t", json!({})), &[0xFE]);
        let cases: &[(&str, Value, &str)] = &[
            (
                "set_level",
                json!({"channel_type": "input", "channel": 1, "level_db": -20.0}),
                "B0 63 40 B0 62 00 B0 06 2E B0 26 40",
            ),
            (
                "set_send_level",
                json!({"channel_type": "input", "channel": 12, "destination_type": "output", "destination": 2, "level_db": -5.0}),
                "B0 63 41 B0 62 49 B0 06 4E B0 26 40",
            ),
            (
                "set_level",
                json!({"channel_type": "output", "channel": 5, "level_db": 5.0}),
                "B0 63 4F B0 62 05 B0 06 73 B0 26 40",
            ),
            (
                "set_send_level",
                json!({"channel_type": "fx_return", "channel": 2, "destination_type": "output", "destination": 1, "level_db": -10.0}),
                "B0 63 46 B0 62 20 B0 06 3E B0 26 00",
            ),
            (
                "set_send_level",
                json!({"channel_type": "fx_return", "channel": 1, "destination_type": "fx_send", "destination": 2, "level_db": -15.0}),
                "B0 63 4E B0 62 05 B0 06 36 B0 26 00",
            ),
            (
                "set_level",
                json!({"channel_type": "dca", "channel": 1, "level_db": -40.0}),
                "B0 63 4F B0 62 20 B0 06 0F B0 26 40",
            ),
            (
                "set_pan",
                json!({"channel_type": "input", "channel": 1, "pan": 0.0}),
                "B0 63 50 B0 62 00 B0 06 40 B0 26 00",
            ),
            (
                "set_send_pan",
                json!({"channel_type": "input", "channel": 3, "destination_type": "output", "destination": 5, "pan": -30.0}),
                "B0 63 50 B0 62 60 B0 06 2C B0 26 65",
            ),
            (
                "set_send_pan",
                json!({"channel_type": "fx_return", "channel": 1, "destination_type": "output", "destination": 1, "pan": 10.0}),
                "B0 63 56 B0 62 14 B0 06 46 B0 26 32",
            ),
            (
                "adjust_send_pan",
                json!({"channel_type": "input", "channel": 10, "destination_type": "output", "destination": 1, "direction": "left"}),
                "B0 63 51 B0 62 30 B0 61 00",
            ),
            (
                "adjust_send_pan",
                json!({"channel_type": "bluetooth", "channel": 1, "destination_type": "output", "destination": 5, "direction": "left"}),
                "B0 63 53 B0 62 30 B0 61 00",
            ),
            (
                "get_send_pan",
                json!({"channel_type": "stereo_input", "channel": 1, "destination_type": "output", "destination": 1}),
                "B0 63 52 B0 62 64 B0 60 7F",
            ),
            (
                "get_level",
                json!({"channel_type": "fx_send", "channel": 2}),
                "B0 63 4F B0 62 0E B0 60 7F",
            ),
            (
                "toggle_mute",
                json!({"channel_type": "input", "channel": 3}),
                "B0 63 00 B0 62 02 B0 60 00",
            ),
            ("recall_scene", json!({"scene": 64}), "B0 00 00 C0 3F"),
            ("soft_key", json!({"key": 3, "pressed": false}), "80 32 00"),
        ];
        for (name, p, want) in cases {
            let got = bytes(&mut m, name, p.clone()).unwrap();
            let want = hex(want);
            assert_eq!(&got[..want.len()], &want[..], "{name} {p}");
        }
        assert!(bytes(
            &mut m,
            "toggle_mute",
            json!({"channel_type": "dca", "channel": 1})
        )
        .is_err());
        assert!(bytes(&mut m, "soft_key", json!({"key": 4, "pressed": true})).is_err());
    }

    #[test]
    fn qu567_examples_byte_for_byte() {
        // Qu-5/6/7 Issue 2 p.13-18, audio taper.
        let cases: &[(i64, &str, Value, &str)] = &[
            (
                1,
                "set_send_level",
                json!({"channel_type": "usb", "channel": 1, "destination_type": "aux", "destination": 5, "level_db": -20.0}),
                "B0 63 43 B0 62 78 B0 06 2E B0 26 40",
            ),
            (
                14,
                "set_send_level",
                json!({"channel_type": "input", "channel": 30, "destination_type": "fx_send", "destination": 3, "level_db": -12.0}),
                "BD 63 4D BD 62 0A BD 06 3B BD 26 00",
            ),
            (
                1,
                "adjust_send_pan",
                json!({"channel_type": "stereo_input", "channel": 2, "destination_type": "aux", "destination": 8, "direction": "right"}),
                "B0 63 53 B0 62 63 B0 60 00",
            ),
            (
                4,
                "set_send_pan",
                json!({"channel_type": "group", "channel": 3, "destination_type": "aux", "destination": 7, "pan": -50.0}),
                "B3 63 55 B3 62 22 B3 06 1F B3 26 7F",
            ),
            (
                1,
                "get_send_pan",
                json!({"channel_type": "input", "channel": 30, "destination_type": "aux", "destination": 5}),
                "B0 63 53 B0 62 24 B0 60 7F",
            ),
            (
                11,
                "set_send_pan",
                json!({"channel_type": "main", "channel": 1, "destination_type": "matrix", "destination": 3, "pan": 100.0}),
                "BA 63 5E BA 62 26 BA 06 7F BA 26 7F",
            ),
            (
                1,
                "set_send_assign",
                json!({"channel_type": "fx_return", "channel": 1, "destination_type": "aux", "destination": 7, "assigned": true}),
                "B0 63 66 B0 62 1A B0 06 00 B0 26 01",
            ),
            (
                1,
                "get_send_assign",
                json!({"channel_type": "fx_return", "channel": 2, "destination_type": "fx_send", "destination": 3}),
                "B0 63 6E B0 62 0A B0 60 7F",
            ),
        ];
        for (channel, name, p, want) in cases {
            let settings = json!({"midi_channel": channel, "fader_law": "audio"});
            let (mut m, _) = connected(sq("qu-6", settings), &[0xFE]);
            let got = bytes(&mut m, name, p.clone()).unwrap();
            let want = hex(want);
            assert_eq!(&got[..want.len()], &want[..], "{name} {p}");
        }
        // Mtx4 and group mutes are not controllable on Qu-5/6/7.
        let (mut m, _) = connected(sq("qu-5", json!({})), &[0xFE]);
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "matrix", "channel": 4, "muted": true})
        )
        .is_err());
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "group", "channel": 1, "muted": true})
        )
        .is_err());
        assert!(bytes(&mut m, "recall_scene", json!({"scene": 300})).is_ok());
    }

    #[test]
    fn replies_and_pushes_become_state() {
        let (mut m, _) = connected(sq("sq-5", json!({})), &[0xFE]);
        let a = run(
            &mut m,
            10,
            "get_send_level",
            json!({"channel_type": "input", "channel": 40,
            "destination_type": "aux", "destination": 5}),
        );
        assert_eq!(sent(&a), [hex("B0 63 44 B0 62 1C B0 60 7F")]);
        // The answer is the set message; linear taper -20 dB is 64 16.
        let a = feed(&mut m, 20, &hex("B0 63 44 62 1C 06 64 26 16"));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!({"level_db": -20.0, "level_raw": (0x64 << 7) | 0x16})
            })
        }));
        let s = state(&feed(
            &mut m,
            30,
            &[
                hex("B0 63 00 62 44 06 00 26 01"),
                hex("B0 63 50 62 17 06 4C 26 65"),
                hex("B0 63 60 62 00 06 00 26 01"),
                hex("B0 00 01 C0 1B"),
                hex("B0 00 0A C0 00"),
            ]
            .concat(),
        ));
        assert_eq!(s["channels"]["main"]["1"]["mute"], true);
        assert_eq!(s["channels"]["input"]["24"]["pan"], 20.0);
        assert_eq!(s["channels"]["input"]["1"]["main_assign"], true);
        assert_eq!(s["scene"]["current"], 156);
        assert_eq!(s["cue"]["current"], 1);
    }

    #[test]
    fn sync_reads_own_values_and_optionally_every_send() {
        let (mut m, a) = connected(sq("sq-7", json!({})), &[0xFE]);
        let own = drain_sync(&mut m, &a).len();
        let (mut m, a) = connected(sq("sq-7", json!({"sync_sends": true})), &[0xFE]);
        let all = drain_sync(&mut m, &a).len();
        assert!(own > 300 && own < 400, "{own}");
        assert!(all > 3000, "{all}");
    }
}
