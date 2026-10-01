//! What the Sony module knows about individual properties and controls: where
//! each operator-facing property goes in the state, how its wire value reads
//! as an operator value and back, and the value type of each control.
//!
//! Codes and value encodings follow Sony's Camera Control PTP 3 Reference;
//! the names are this module's own. A property without an entry here is still
//! reported under `properties.<code>` and can be written with `set_property`.

use serde_json::{json, Value};

use super::sony_camera_dataset::{dt, PtpValue};

/// How a property's wire value reads in the state.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Decode {
    /// The integer or string as the camera sends it.
    Raw,
    /// Named values; an unnamed value reads as its hex code.
    Labels(&'static [(i128, &'static str)]),
    /// 1 off, 2 on.
    OnOff,
    /// 0 off, 1 on.
    Flag,
    /// The value divided by this.
    Scaled(f64),
    /// F- or T-number in hundredths; 0xFFFD is a closed iris.
    FNumber,
    /// Shutter speed as a 16-bit numerator over a 16-bit denominator.
    Shutter32,
    /// Shutter speed as a 32-bit numerator over a 32-bit denominator.
    Shutter64,
    /// ISO in the low 24 bits, with 0xFFFFFF for auto.
    Iso,
    /// A percentage where -1 means the camera has no reading.
    Percent,
    /// A count where all ones means the camera has no reading.
    Count32,
    /// Thousandths of a volt, all ones meaning no reading.
    Millivolts,
    /// Millionths of a degree.
    Microdegrees,
    /// Hours, minutes, seconds and frames, one byte each from the top.
    Timecode,
    /// Four user-bit bytes as eight hex digits.
    UserBits,
    /// A 32-bit numerator over a 32-bit denominator, as a fraction.
    Ratio64,
    /// A 16-bit numerator over a 16-bit denominator, as a percentage.
    Progress,
    /// Movie frame rate code.
    FrameRate,
}

/// One operator-facing property.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Prop {
    pub code: u16,
    /// Dotted state path.
    pub path: &'static str,
    pub decode: Decode,
    /// The typed command that writes it, taking `value`.
    pub command: Option<&'static str>,
}

const fn p(code: u16, path: &'static str, decode: Decode, command: Option<&'static str>) -> Prop {
    Prop {
        code,
        path,
        decode,
        command,
    }
}

const AUTO_MANUAL: &[(i128, &str)] = &[(1, "auto"), (2, "manual")];

const EXPOSURE_MODE: &[(i128, &str)] = &[
    (0x0000_0001, "manual"),
    (0x0001_0002, "program"),
    (0x0002_0003, "aperture_priority"),
    (0x0003_0004, "shutter_priority"),
    (0x0000_0005, "program_creative"),
    (0x0000_0006, "program_action"),
    (0x0000_0007, "portrait"),
    (0x0004_8000, "auto"),
    (0x0004_8001, "auto_plus"),
    (0x0000_8008, "program_aperture"),
    (0x0000_8009, "program_shutter"),
    (0x0005_8011, "scene_sports"),
    (0x0005_8012, "scene_sunset"),
    (0x0005_8013, "scene_night"),
    (0x0005_8014, "scene_landscape"),
    (0x0005_8015, "scene_macro"),
    (0x0005_8016, "scene_handheld_twilight"),
    (0x0005_8017, "scene_night_portrait"),
    (0x0005_8018, "scene_anti_motion_blur"),
    (0x0005_8019, "scene_pet"),
    (0x0005_801A, "scene_food"),
    (0x0005_801B, "scene_fireworks"),
    (0x0005_801C, "scene_high_sensitivity"),
    (0x0000_8020, "memory_recall"),
    (0x0000_8030, "continuous_priority_ae"),
    (0x0000_8031, "tele_zoom_continuous_8"),
    (0x0000_8032, "tele_zoom_continuous_10"),
    (0x0000_8033, "continuous_priority_ae_12"),
    (0x0006_8040, "sweep_panorama_3d"),
    (0x0006_8041, "sweep_panorama"),
    (0x0007_8050, "movie_program"),
    (0x0007_8051, "movie_aperture_priority"),
    (0x0007_8052, "movie_shutter_priority"),
    (0x0007_8053, "movie_manual"),
    (0x0007_8054, "movie_auto"),
    (0x0009_8059, "movie_sq_program"),
    (0x0009_805A, "movie_sq_aperture_priority"),
    (0x0009_805B, "movie_sq_shutter_priority"),
    (0x0009_805C, "movie_sq_manual"),
    (0x0009_805D, "movie_sq_auto"),
    (0x0000_8060, "flash_off"),
    (0x0000_8070, "picture_effect"),
    (0x0008_8080, "hfr_program"),
    (0x0008_8081, "hfr_aperture_priority"),
    (0x0008_8082, "hfr_shutter_priority"),
    (0x0008_8083, "hfr_manual"),
    (0x0000_8084, "sq_program"),
    (0x0000_8085, "sq_aperture_priority"),
    (0x0000_8086, "sq_shutter_priority"),
    (0x0000_8087, "sq_manual"),
    (0x000A_8088, "movie"),
    (0x000A_8089, "still"),
    (0x000B_808A, "flexible_movie_or_sq"),
    (0x0007_8090, "movie_flexible"),
    (0x0009_8091, "sq_flexible"),
    (0x000C_8092, "interval_movie_flexible"),
    (0x000C_8093, "interval_movie_program"),
    (0x000C_8094, "interval_movie_aperture_priority"),
    (0x000C_8095, "interval_movie_shutter_priority"),
    (0x000C_8096, "interval_movie_manual"),
    (0x000C_8097, "interval_movie_auto"),
];

const WHITE_BALANCE: &[(i128, &str)] = &[
    (0x0001, "manual"),
    (0x0002, "auto"),
    (0x0003, "one_push_auto"),
    (0x0004, "daylight"),
    (0x0005, "fluorescent"),
    (0x0006, "tungsten"),
    (0x0007, "flash"),
    (0x8001, "fluorescent_warm_white"),
    (0x8002, "fluorescent_cool_white"),
    (0x8003, "fluorescent_day_white"),
    (0x8004, "fluorescent_daylight"),
    (0x8010, "cloudy"),
    (0x8011, "shade"),
    (0x8012, "color_temperature"),
    (0x8020, "custom_1"),
    (0x8021, "custom_2"),
    (0x8022, "custom_3"),
    (0x8023, "custom"),
    (0x8030, "underwater_auto"),
];

const METERING: &[(i128, &str)] = &[
    (0x0001, "average"),
    (0x0002, "center_weighted_average"),
    (0x0003, "multi_spot"),
    (0x0004, "center_spot"),
    (0x8001, "multi"),
    (0x8002, "center_weighted"),
    (0x8003, "entire_screen_average"),
    (0x8004, "spot_standard"),
    (0x8005, "spot_large"),
    (0x8006, "highlight"),
    (0x8011, "standard"),
    (0x8012, "backlight"),
    (0x8013, "spotlight"),
];

const FOCUS_MODE: &[(i128, &str)] = &[
    (0x0001, "manual"),
    (0x0002, "af_single"),
    (0x0003, "af_macro"),
    (0x8004, "af_continuous"),
    (0x8005, "af_auto"),
    (0x8006, "direct_manual"),
    (0x8007, "manual_reverse"),
    (0x8008, "af_d"),
    (0x8009, "preset_focus"),
];

const FOCUS_AREA: &[(i128, &str)] = &[
    (0x0000, "unknown"),
    (0x0001, "wide"),
    (0x0002, "zone"),
    (0x0003, "center"),
    (0x0101, "flexible_spot_s"),
    (0x0102, "flexible_spot_m"),
    (0x0103, "flexible_spot_l"),
    (0x0104, "expand_flexible_spot"),
    (0x0105, "flexible_spot"),
    (0x0106, "flexible_spot_xs"),
    (0x0107, "flexible_spot_xl"),
    (0x1101, "flexible_spot_free_1"),
    (0x1102, "flexible_spot_free_2"),
    (0x1103, "flexible_spot_free_3"),
    (0x0201, "tracking_wide"),
    (0x0202, "tracking_zone"),
    (0x0203, "tracking_center"),
    (0x0204, "tracking_flexible_spot_s"),
    (0x0205, "tracking_flexible_spot_m"),
    (0x0206, "tracking_flexible_spot_l"),
    (0x0207, "tracking_expand_flexible_spot"),
    (0x0208, "tracking_flexible_spot"),
    (0x0209, "tracking_flexible_spot_xs"),
    (0x020A, "tracking_flexible_spot_xl"),
    (0x1201, "tracking_flexible_spot_free_1"),
    (0x1202, "tracking_flexible_spot_free_2"),
    (0x1203, "tracking_flexible_spot_free_3"),
];

pub(crate) const FOCUS_INDICATION: &[(i128, &str)] = &[
    (0x01, "unlocked"),
    (0x02, "single_focused"),
    (0x03, "single_not_focused"),
    (0x04, "unused"),
    (0x05, "continuous_tracking"),
    (0x06, "continuous_focused"),
    (0x07, "continuous_not_focused"),
    (0x08, "unpaused"),
    (0x09, "paused"),
];

const FOCUS_STATUS: &[(i128, &str)] = &[(1, "hold"), (2, "manual"), (3, "auto")];

const SUBJECT_RECOGNITION: &[(i128, &str)] = &[(1, "off"), (2, "af_only"), (3, "af_priority")];

const RECORDING_STATE: &[(i128, &str)] = &[
    (0, "not_recording"),
    (1, "recording"),
    (2, "failed"),
    (3, "waiting"),
];

const RECORDER_STATUS: &[(i128, &str)] = &[
    (0, "idle"),
    (1, "ready"),
    (2, "preparing"),
    (3, "standby"),
    (4, "recording"),
    (5, "stopping"),
];

const MOVIE_FORMAT: &[(i128, &str)] = &[
    (0x01, "dvd"),
    (0x02, "m2ps"),
    (0x03, "avchd"),
    (0x04, "mp4"),
    (0x05, "dv"),
    (0x06, "xavc"),
    (0x07, "mxf"),
    (0x08, "xavc_s_4k"),
    (0x09, "xavc_s_hd"),
    (0x0A, "xavc_hs_8k"),
    (0x0B, "xavc_hs_4k"),
    (0x0C, "xavc_s_l_4k"),
    (0x0D, "xavc_s_l_hd"),
    (0x0E, "xavc_s_i_4k"),
    (0x0F, "xavc_s_i_hd"),
    (0x10, "xavc_i"),
    (0x11, "xavc_l"),
    (0x12, "xavc_proxy"),
    (0x13, "xavc_hs_hd"),
    (0x14, "xavc_s_i_dci_4k"),
    (0x15, "xavc_h_i_hq"),
    (0x16, "xavc_h_i_sq"),
    (0x17, "xavc_h_l"),
    (0x18, "x_ocn_xt"),
    (0x19, "x_ocn_st"),
    (0x1A, "x_ocn_lt"),
    (0x1B, "xavc_hs_l_422"),
    (0x1C, "xavc_hs_l_420"),
    (0x1D, "xavc_s_l_422"),
    (0x1E, "xavc_s_l_420"),
    (0x1F, "xavc_s_i_422"),
    (0x20, "mpeg_hd_422"),
];

/// Frame rates by code; progressive from 0x01, interlaced from 0x41.
const FRAME_RATES: [&str; 23] = [
    "120", "100", "60", "50", "30", "25", "24", "23.98", "29.97", "59.94", "19.98", "14.99",
    "12.50", "12.00", "11.99", "10.00", "9.99", "6.00", "5.99", "5.00", "4.995", "24.00", "119.88",
];

const MEDIA_STATUS: &[(i128, &str)] = &[
    (1, "ok"),
    (2, "no_card"),
    (3, "card_error"),
    (4, "recognizing_or_locked"),
    (5, "database_error"),
    (6, "recognizing"),
    (7, "locked_database_error"),
    (8, "needs_format"),
    (9, "read_only"),
];

const REC_AVAILABLE: &[(i128, &str)] = &[
    (0, "none"),
    (1, "main"),
    (2, "proxy"),
    (3, "main_and_proxy"),
];

const BATTERY_LEVEL: &[(i128, &str)] = &[
    (0x01, "not_genuine"),
    (0x02, "unusable"),
    (0x03, "nearly_empty"),
    (0x04, "1_of_4"),
    (0x05, "2_of_4"),
    (0x06, "3_of_4"),
    (0x07, "4_of_4"),
    (0x08, "1_of_3"),
    (0x09, "2_of_3"),
    (0x0A, "3_of_3"),
    (0x0B, "nearly_empty_usb_powered"),
    (0x0C, "1_of_4_usb_powered"),
    (0x0D, "2_of_4_usb_powered"),
    (0x0E, "3_of_4_usb_powered"),
    (0x0F, "4_of_4_usb_powered"),
    (0x10, "usb_powered"),
    (0xFF, "no_battery"),
];

const POWER_SOURCE: &[(i128, &str)] = &[(1, "dc"), (2, "battery"), (3, "poe")];

const POWER_STATUS: &[(i128, &str)] = &[
    (1, "off"),
    (2, "standby"),
    (3, "on"),
    (4, "going_to_standby"),
    (5, "powering_on"),
];

const OVERHEATING: &[(i128, &str)] = &[(0, "normal"), (1, "warning"), (2, "overheating")];

const DRIVE_MODE: &[(i128, &str)] = &[
    (0x0000_0001, "single"),
    (0x0001_0002, "continuous_hi"),
    (0x0001_8010, "continuous_hi_plus"),
    (0x0001_8011, "continuous_hi_live"),
    (0x0001_8012, "continuous_lo"),
    (0x0001_8013, "continuous"),
    (0x0001_8014, "continuous_speed_priority"),
    (0x0001_8015, "continuous_mid"),
    (0x0001_8016, "continuous_mid_live"),
    (0x0001_8017, "continuous_lo_live"),
    (0x0002_0003, "timelapse"),
    (0x0003_8003, "self_timer_5s"),
    (0x0003_8004, "self_timer_10s"),
    (0x0003_8005, "self_timer_2s"),
    (0x0007_800A, "remote_commander"),
    (0x0007_800B, "mirror_up"),
    (0x0007_8006, "self_portrait_1"),
    (0x0007_8007, "self_portrait_2"),
    (0x0008_8008, "self_timer_continuous_3"),
    (0x0008_8009, "self_timer_continuous_5"),
    (0x0008_800C, "self_timer_continuous_3_5s"),
    (0x0008_800D, "self_timer_continuous_5_5s"),
    (0x0008_800E, "self_timer_continuous_3_2s"),
    (0x0008_800F, "self_timer_continuous_5_2s"),
    (0x0009_8030, "spot_burst_lo"),
    (0x0009_8031, "spot_burst_mid"),
    (0x0009_8032, "spot_burst_hi"),
];

const STILL_FORMAT: &[(i128, &str)] = &[
    (1, "raw"),
    (2, "raw_jpeg"),
    (3, "jpeg"),
    (4, "raw_heif"),
    (5, "heif"),
];

const STILL_QUALITY: &[(i128, &str)] = &[
    (1, "extra_fine"),
    (2, "fine"),
    (3, "standard"),
    (4, "light"),
];

const ASPECT_RATIO: &[(i128, &str)] = &[(1, "3:2"), (2, "16:9"), (3, "4:3"), (4, "1:1")];

const PICTURE_PROFILE: &[(i128, &str)] = &[
    (0x00, "off"),
    (0x01, "pp1"),
    (0x02, "pp2"),
    (0x03, "pp3"),
    (0x04, "pp4"),
    (0x05, "pp5"),
    (0x06, "pp6"),
    (0x07, "pp7"),
    (0x08, "pp8"),
    (0x09, "pp9"),
    (0x0A, "pp10"),
    (0x0B, "pp11"),
    (0x41, "pp_lut1"),
    (0x42, "pp_lut2"),
    (0x43, "pp_lut3"),
    (0x44, "pp_lut4"),
];

const GAMMA: &[(i128, &str)] = &[
    (0x0001, "movie"),
    (0x0002, "still"),
    (0x0003, "s_cinetone"),
    (0x0101, "cine1"),
    (0x0102, "cine2"),
    (0x0103, "cine3"),
    (0x0104, "cine4"),
    (0x0201, "itu709"),
    (0x0202, "itu709_800"),
    (0x0302, "s_log2"),
    (0x0303, "s_log3"),
    (0x0401, "hlg"),
    (0x0402, "hlg1"),
    (0x0403, "hlg2"),
    (0x0404, "hlg3"),
];

const CREATIVE_LOOK: &[(i128, &str)] = &[
    (0x0001, "st"),
    (0x0002, "pt"),
    (0x0003, "nt"),
    (0x0004, "vv"),
    (0x0005, "vv2"),
    (0x0006, "fl"),
    (0x0007, "in"),
    (0x0008, "sh"),
    (0x0009, "bw"),
    (0x000A, "se"),
    (0x000B, "fl2"),
    (0x000C, "fl3"),
    (0x0101, "custom_1"),
    (0x0102, "custom_2"),
    (0x0103, "custom_3"),
    (0x0104, "custom_4"),
    (0x0105, "custom_5"),
    (0x0106, "custom_6"),
];

const CREATIVE_STYLE: &[(i128, &str)] = &[
    (0x01, "standard"),
    (0x02, "vivid"),
    (0x03, "portrait"),
    (0x04, "landscape"),
    (0x05, "sunset"),
    (0x06, "black_and_white"),
    (0x07, "light"),
    (0x08, "neutral"),
    (0x09, "clear"),
    (0x0A, "deep"),
    (0x0B, "night_view"),
    (0x0C, "autumn_leaves"),
    (0x0D, "sepia"),
    (0x0E, "creative_box_1"),
    (0x0F, "creative_box_2"),
    (0x10, "creative_box_3"),
    (0x11, "creative_box_4"),
    (0x12, "creative_box_5"),
    (0x13, "creative_box_6"),
];

const DRO: &[(i128, &str)] = &[
    (0x01, "off"),
    (0x02, "dro"),
    (0x10, "dro_plus"),
    (0x11, "dro_manual_1"),
    (0x12, "dro_manual_2"),
    (0x13, "dro_manual_3"),
    (0x14, "dro_manual_4"),
    (0x15, "dro_manual_5"),
    (0x16, "dro_manual_6"),
    (0x17, "dro_manual_7"),
    (0x18, "dro_manual_8"),
    (0x1F, "dro_auto"),
    (0x20, "hdr_auto"),
    (0x21, "hdr_1ev"),
    (0x22, "hdr_2ev"),
    (0x23, "hdr_3ev"),
    (0x24, "hdr_4ev"),
    (0x25, "hdr_5ev"),
    (0x26, "hdr_6ev"),
];

const STEADY_SHOT_MOVIE: &[(i128, &str)] =
    &[(1, "off"), (2, "standard"), (3, "active"), (4, "hybrid")];

const ZOOM_SETTING: &[(i128, &str)] = &[
    (1, "optical_only"),
    (2, "smart_only"),
    (3, "clear_image_zoom"),
    (4, "digital_zoom"),
];

const ZOOM_TYPE: &[(i128, &str)] = &[
    (1, "optical"),
    (2, "smart"),
    (3, "clear_image"),
    (4, "digital"),
];

const ZOOM_SPEED_TYPE: &[(i128, &str)] = &[(0, "invalid"), (1, "variable"), (2, "fixed")];

const ND_MODE: &[(i128, &str)] = &[
    (1, "auto"),
    (2, "preset"),
    (3, "preset_clear"),
    (4, "variable"),
    (5, "variable_clear"),
    (6, "step"),
    (7, "step_clear"),
];

const SHUTTER_MODE: &[(i128, &str)] = &[(1, "speed"), (2, "angle")];
const GAIN_UNIT: &[(i128, &str)] = &[(1, "db"), (2, "iso")];
const HIGH_LOW: &[(i128, &str)] = &[(1, "high"), (2, "low")];
const EXPOSURE_CONTROL: &[(i128, &str)] = &[(1, "pasm"), (2, "flexible")];
const WB_MEMORY: &[(i128, &str)] = &[(1, "preset"), (2, "memory_a"), (3, "memory_b")];
const TC_FORMAT: &[(i128, &str)] = &[(1, "df"), (2, "ndf")];
const TC_RUN: &[(i128, &str)] = &[(1, "rec_run"), (2, "free_run")];
const TC_MAKE: &[(i128, &str)] = &[(1, "preset"), (2, "regenerate")];
const STREAM_STATUS: &[(i128, &str)] =
    &[(1, "inactive"), (2, "idle"), (3, "streaming"), (4, "error")];
const PT_STATUS: &[(i128, &str)] = &[
    (1, "idle"),
    (2, "moving"),
    (3, "unknown"),
    (4, "uninitialized"),
];
const OPERATING_MODE: &[(i128, &str)] = &[(1, "record"), (2, "playback")];
const MENU: &[(i128, &str)] = &[(1, "off"), (2, "status_menu"), (3, "full_menu")];

const MOVIE_SHOOTING_MODE: &[(i128, &str)] = &[
    (0x0001, "off"),
    (0x0101, "sdr"),
    (0x0201, "hdr"),
    (0x0301, "cine_ei"),
    (0x0302, "cine_ei_quick"),
    (0x0401, "custom"),
    (0x0501, "flexible_iso"),
];

pub(crate) const AUDIO_INPUT: &[(i128, &str)] = &[
    (1, "off"),
    (2, "input1"),
    (3, "input2"),
    (4, "internal_mic"),
    (5, "shoe_ch1"),
    (6, "shoe_ch2"),
    (7, "mic_jack_left"),
    (8, "mic_jack_right"),
];

const LIVE_VIEW_QUALITY: &[(i128, &str)] = &[(1, "low"), (2, "high")];
const FTP_STATUS: &[(i128, &str)] = &[
    (1, "connecting"),
    (2, "connected"),
    (3, "connected_certificate_error"),
    (4, "error"),
];
const FTP_ERROR: &[(i128, &str)] = &[
    (0x00, "none"),
    (0x01, "camera_system_error"),
    (0x02, "wifi_hardware_error"),
    (0x03, "wired_lan_hardware_error"),
    (0x04, "access_point_not_registered"),
    (0x05, "access_point_not_found"),
    (0x06, "access_point_connection_error"),
    (0x07, "access_point_password_error"),
    (0x08, "wep_key_or_static_ip_error"),
    (0x09, "wep_key_or_ip_address_error"),
    (0x0A, "dhcp_error"),
    (0x0B, "dns_error"),
    (0x0C, "airplane_mode"),
    (0x0D, "lan_cable_error"),
    (0x0E, "server_not_set"),
    (0x0F, "server_login_error"),
    (0x10, "server_disconnected"),
    (0x11, "certificate_error"),
    (0x12, "directory_create_error"),
    (0x13, "permission_or_capacity_error"),
    (0x14, "usb_lan_adapter_not_recognised"),
    (0x15, "usb_tethering_not_recognised"),
    (0x16, "check_connected_device"),
    (0x17, "reconnecting_to_server"),
    (0x18, "transfer_failed_reconnecting"),
    (0xFFFF, "unknown"),
];
const FTP_AUTO_TARGET: &[(i128, &str)] = &[(1, "still"), (2, "movie"), (3, "still_and_movie")];
const FTP_STILL_TARGET: &[(i128, &str)] = &[(1, "all"), (2, "protected_only")];
const FTP_MOVIE_TARGET: &[(i128, &str)] =
    &[(1, "all"), (2, "shot_mark_only"), (3, "protected_only")];
const FTP_FILE_TARGET: &[(i128, &str)] = &[
    (1, "jpeg_heif_only"),
    (2, "raw_only"),
    (3, "raw_and_jpeg_heif"),
];
const FTP_PROXY_TARGET: &[(i128, &str)] = &[
    (1, "proxy_only"),
    (2, "original_only"),
    (3, "proxy_and_original"),
];
const FTP_STILL_SIZE: &[(i128, &str)] = &[(1, "small"), (2, "large")];

use Decode::*;

/// The operator-facing properties: their state paths, readings and typed
/// commands.
pub(crate) const PROPS: &[Prop] = &[
    // Exposure
    p(
        0x500E,
        "exposure.mode",
        Labels(EXPOSURE_MODE),
        Some("set_exposure_mode"),
    ),
    p(
        0xD099,
        "exposure.control_type",
        Labels(EXPOSURE_CONTROL),
        Some("set_exposure_control_type"),
    ),
    p(0x5007, "exposure.iris", FNumber, Some("set_iris")),
    p(0xD000, "exposure.t_number", FNumber, None),
    p(
        0xD001,
        "exposure.iris_control",
        Labels(AUTO_MANUAL),
        Some("set_iris_control"),
    ),
    p(0xD20D, "exposure.shutter_speed", Shutter32, None),
    p(0xD016, "exposure.shutter_speed_setting", Shutter64, None),
    p(0xD017, "exposure.shutter_speed_current", Shutter64, None),
    p(
        0xD00E,
        "exposure.shutter_angle",
        Scaled(1000.0),
        Some("set_shutter_angle"),
    ),
    p(
        0xD010,
        "exposure.shutter_mode",
        Labels(SHUTTER_MODE),
        Some("set_shutter_mode"),
    ),
    p(
        0xD013,
        "exposure.shutter_control",
        Labels(AUTO_MANUAL),
        Some("set_shutter_control"),
    ),
    p(
        0xD00F,
        "exposure.shutter_enabled",
        OnOff,
        Some("set_shutter_enabled"),
    ),
    p(0xD21E, "exposure.iso", Iso, Some("set_iso")),
    p(0xD023, "exposure.iso_current", Iso, None),
    p(
        0xD022,
        "exposure.exposure_index",
        Raw,
        Some("set_exposure_index"),
    ),
    p(0xD01E, "exposure.gain_db", Raw, Some("set_gain_db")),
    p(0xD01F, "exposure.gain_db_current", Raw, None),
    p(
        0xD01C,
        "exposure.gain_control",
        Labels(AUTO_MANUAL),
        Some("set_gain_control"),
    ),
    p(
        0xD01D,
        "exposure.gain_unit",
        Labels(GAIN_UNIT),
        Some("set_gain_unit"),
    ),
    p(
        0xD020,
        "exposure.base_iso",
        Labels(HIGH_LOW),
        Some("set_base_iso"),
    ),
    p(
        0xD021,
        "exposure.base_sensitivity",
        Labels(HIGH_LOW),
        Some("set_base_sensitivity"),
    ),
    p(
        0x5010,
        "exposure.compensation",
        Scaled(1000.0),
        Some("set_exposure_compensation"),
    ),
    p(
        0x500B,
        "exposure.metering",
        Labels(METERING),
        Some("set_metering_mode"),
    ),
    p(
        0xD200,
        "exposure.flash_compensation",
        Scaled(1000.0),
        Some("set_flash_compensation"),
    ),
    p(0xD237, "exposure.step", Scaled(100.0), None),
    p(0xD217, "exposure.ae_locked", OnOff, None),
    p(
        0xD1B5,
        "exposure.metered_manual_level",
        Scaled(1000.0),
        None,
    ),
    p(0xD201, "exposure.dro", Labels(DRO), Some("set_dro")),
    // ND filter
    p(0xD018, "nd.enabled", OnOff, Some("set_nd_filter")),
    p(0xD019, "nd.mode", Labels(ND_MODE), None),
    p(
        0xD01A,
        "nd.control",
        Labels(AUTO_MANUAL),
        Some("set_nd_control"),
    ),
    p(0xD01B, "nd.transmittance", Ratio64, None),
    // White balance
    p(
        0x5005,
        "white_balance.mode",
        Labels(WHITE_BALANCE),
        Some("set_white_balance"),
    ),
    p(
        0xD00C,
        "white_balance.control",
        Labels(AUTO_MANUAL),
        Some("set_white_balance_control"),
    ),
    p(
        0xD20F,
        "white_balance.color_temperature",
        Raw,
        Some("set_color_temperature"),
    ),
    p(0xD00D, "white_balance.tint", Raw, Some("set_tint")),
    p(0xD210, "white_balance.green_magenta", Raw, None),
    p(0xD21C, "white_balance.amber_blue", Raw, None),
    p(
        0xD085,
        "white_balance.memory",
        Labels(WB_MEMORY),
        Some("set_white_balance_memory"),
    ),
    p(
        0xD086,
        "white_balance.preset_color_temperature",
        Raw,
        Some("set_preset_color_temperature"),
    ),
    p(
        0xD087,
        "white_balance.r_gain",
        Scaled(10.0),
        Some("set_white_balance_r_gain"),
    ),
    p(
        0xD088,
        "white_balance.b_gain",
        Scaled(10.0),
        Some("set_white_balance_b_gain"),
    ),
    p(0xD24E, "white_balance.awb_locked", OnOff, None),
    // Focus
    p(
        0x500A,
        "focus.mode",
        Labels(FOCUS_MODE),
        Some("set_focus_mode"),
    ),
    p(
        0xD007,
        "focus.control",
        Labels(AUTO_MANUAL),
        Some("set_focus_control"),
    ),
    p(
        0xD22C,
        "focus.area",
        Labels(FOCUS_AREA),
        Some("set_focus_area"),
    ),
    p(0xD213, "focus.indication", Labels(FOCUS_INDICATION), None),
    p(0xE044, "focus.status", Labels(FOCUS_STATUS), None),
    p(0xE043, "focus.position", Raw, None),
    p(0xE042, "focus.position_target", Raw, None),
    p(0xD24C, "focus.focal_position", Raw, None),
    p(0xD004, "focus.distance", Raw, None),
    p(0xD19C, "focus.driving", OnOff, None),
    p(
        0xD060,
        "focus.subject_recognition",
        Labels(SUBJECT_RECOGNITION),
        Some("set_subject_recognition_af"),
    ),
    p(
        0xD061,
        "focus.af_transition_speed",
        Raw,
        Some("set_af_transition_speed"),
    ),
    p(
        0xD062,
        "focus.af_subject_shift_sensitivity",
        Raw,
        Some("set_af_subject_shift_sensitivity"),
    ),
    // Zoom
    p(0xD25C, "zoom.scale", Scaled(1000.0), None),
    p(0xD00A, "zoom.digital_scale", Scaled(1000.0), None),
    p(0xD00B, "zoom.distance_mm", Scaled(1000.0), None),
    p(0xE041, "zoom.position", Raw, None),
    p(0xE040, "zoom.position_target", Raw, None),
    p(
        0xD25F,
        "zoom.setting",
        Labels(ZOOM_SETTING),
        Some("set_zoom_setting"),
    ),
    p(0xD260, "zoom.type", Labels(ZOOM_TYPE), None),
    p(0xD25B, "zoom.operation_enabled", Flag, None),
    p(
        0xD299,
        "zoom.remote_speed_type",
        Labels(ZOOM_SPEED_TYPE),
        Some("set_remote_zoom_speed_type"),
    ),
    p(0xD19D, "zoom.driving", OnOff, None),
    // Movie recording
    p(0xD21D, "recording.state", Labels(RECORDING_STATE), None),
    p(0xD120, "recording.duration", Raw, None),
    p(
        0xE010,
        "recording.main_status",
        Labels(RECORDER_STATUS),
        None,
    ),
    p(
        0xE011,
        "recording.proxy_status",
        Labels(RECORDER_STATUS),
        None,
    ),
    p(0xE00B, "recording.clip_name", Raw, None),
    p(
        0xD241,
        "recording.file_format",
        Labels(MOVIE_FORMAT),
        Some("set_movie_file_format"),
    ),
    p(
        0xD242,
        "recording.setting",
        Raw,
        Some("set_movie_recording_setting"),
    ),
    p(
        0xD286,
        "recording.frame_rate",
        FrameRate,
        Some("set_movie_frame_rate"),
    ),
    p(0xD109, "recording.proxy_setting", Raw, None),
    p(0xD050, "recording.simul_rec", OnOff, Some("set_simul_rec")),
    p(0xD0D2, "recording.audio", Flag, Some("set_audio_recording")),
    p(
        0xD051,
        "recording.slow_and_quick",
        OnOff,
        Some("set_slow_and_quick"),
    ),
    p(
        0xD052,
        "recording.slow_and_quick_frame_rate",
        Raw,
        Some("set_slow_and_quick_frame_rate"),
    ),
    p(
        0xE000,
        "recording.shooting_mode",
        Labels(MOVIE_SHOOTING_MODE),
        Some("set_movie_shooting_mode"),
    ),
    // Stills
    p(
        0x5013,
        "still.drive_mode",
        Labels(DRIVE_MODE),
        Some("set_drive_mode"),
    ),
    p(
        0xD253,
        "still.file_format",
        Labels(STILL_FORMAT),
        Some("set_still_file_format"),
    ),
    p(
        0xD252,
        "still.quality",
        Labels(STILL_QUALITY),
        Some("set_still_quality"),
    ),
    p(0xD203, "still.image_size", Raw, None),
    p(
        0xD211,
        "still.aspect_ratio",
        Labels(ASPECT_RATIO),
        Some("set_aspect_ratio"),
    ),
    p(0xD215, "still.pending_files", Raw, None),
    // Image
    p(
        0xD23F,
        "image.picture_profile",
        Labels(PICTURE_PROFILE),
        Some("set_picture_profile"),
    ),
    p(0xD0E1, "image.gamma", Labels(GAMMA), Some("set_gamma")),
    p(
        0xD0FA,
        "image.creative_look",
        Labels(CREATIVE_LOOK),
        Some("set_creative_look"),
    ),
    p(
        0xD240,
        "image.creative_style",
        Labels(CREATIVE_STYLE),
        Some("set_creative_style"),
    ),
    p(0xD03C, "image.base_look", Raw, Some("set_base_look")),
    p(0xD0D9, "image.steady_shot", OnOff, Some("set_steady_shot")),
    p(
        0xD0DA,
        "image.steady_shot_movie",
        Labels(STEADY_SHOT_MOVIE),
        Some("set_steady_shot_movie"),
    ),
    // Timecode
    p(
        0xD0D3,
        "timecode.preset",
        Timecode,
        Some("set_timecode_preset"),
    ),
    p(
        0xD0D4,
        "timecode.user_bits_preset",
        UserBits,
        Some("set_user_bits_preset"),
    ),
    p(
        0xD0D5,
        "timecode.format",
        Labels(TC_FORMAT),
        Some("set_timecode_format"),
    ),
    p(
        0xD0D6,
        "timecode.run",
        Labels(TC_RUN),
        Some("set_timecode_run"),
    ),
    p(
        0xD0D7,
        "timecode.make",
        Labels(TC_MAKE),
        Some("set_timecode_make"),
    ),
    p(
        0xD0D8,
        "timecode.user_bits_time_rec",
        OnOff,
        Some("set_user_bits_time_rec"),
    ),
    // Tally lamps (pan/tilt cameras)
    p(0xE0D2, "tally.red", OnOff, None),
    p(0xE0D3, "tally.green", OnOff, None),
    p(0xE0D4, "tally.yellow", OnOff, None),
    // Media
    p(0xD248, "media.slot1.status", Labels(MEDIA_STATUS), None),
    p(0xD249, "media.slot1.remaining_shots", Count32, None),
    p(0xD24A, "media.slot1.remaining_seconds", Count32, None),
    p(0xD197, "media.slot1.writing", OnOff, None),
    p(0xD279, "media.slot1.format_available", Flag, None),
    p(
        0xD02B,
        "media.slot1.recordable",
        Labels(REC_AVAILABLE),
        None,
    ),
    p(0xD256, "media.slot2.status", Labels(MEDIA_STATUS), None),
    p(0xD257, "media.slot2.remaining_shots", Count32, None),
    p(0xD258, "media.slot2.remaining_seconds", Count32, None),
    p(0xD198, "media.slot2.writing", OnOff, None),
    p(0xD27A, "media.slot2.format_available", Flag, None),
    p(
        0xD02C,
        "media.slot2.recordable",
        Labels(REC_AVAILABLE),
        None,
    ),
    p(0xD18E, "media.slot3.status", Labels(MEDIA_STATUS), None),
    p(0xD18F, "media.slot3.remaining_seconds", Count32, None),
    p(
        0xD190,
        "media.slot3.recordable",
        Labels(REC_AVAILABLE),
        None,
    ),
    p(0xD27B, "media.format_progress", Progress, None),
    // Power
    p(0xD03A, "power.source", Labels(POWER_SOURCE), None),
    p(0xE0AF, "power.status", Labels(POWER_STATUS), None),
    p(0xD218, "power.battery.percent", Percent, None),
    p(0xD20E, "power.battery.level", Labels(BATTERY_LEVEL), None),
    p(0xD038, "power.battery.minutes", Count32, None),
    p(0xD039, "power.battery.volts", Millivolts, None),
    p(0xD204, "power.battery.total_percent", Percent, None),
    p(
        0xD205,
        "power.battery.total_level",
        Labels(BATTERY_LEVEL),
        None,
    ),
    p(0xD12D, "power.battery.second_percent", Percent, None),
    p(0xD03E, "power.dc_volts", Millivolts, None),
    p(0xD251, "power.overheating", Labels(OVERHEATING), None),
    // Lens and body
    p(0xD07B, "lens.model", Raw, None),
    p(0xD07C, "lens.serial", Raw, None),
    p(0xD07D, "lens.version", Raw, None),
    p(0xD040, "device.software_version", Raw, None),
    p(
        0xD0BC,
        "status.operating_mode",
        Labels(OPERATING_MODE),
        None,
    ),
    p(0xD264, "status.remote_restricted", Flag, None),
    p(0xD1BB, "status.camera_error", OnOff, None),
    p(0xD1BC, "status.system_error", OnOff, None),
    p(0xD07A, "status.system_error_info", Raw, None),
    p(0xD080, "status.menu", Labels(MENU), None),
    // Monitoring aids
    p(0xE11A, "monitoring.peaking", OnOff, Some("set_peaking")),
    p(0xE11D, "monitoring.zebra", OnOff, Some("set_zebra")),
    p(
        0xE11E,
        "monitoring.zebra_level",
        Raw,
        Some("set_zebra_level"),
    ),
    p(0xE123, "monitoring.marker", OnOff, Some("set_marker")),
    p(0xD1DE, "monitoring.grid_line", OnOff, Some("set_grid_line")),
    p(
        0xD1FE,
        "monitoring.gamma_assist",
        OnOff,
        Some("set_gamma_display_assist"),
    ),
    p(0xD04D, "monitoring.lut", OnOff, Some("set_monitor_lut")),
    // Audio
    p(0xE048, "audio.ch1.level_control", Labels(AUTO_MANUAL), None),
    p(0xE049, "audio.ch2.level_control", Labels(AUTO_MANUAL), None),
    p(0xE04A, "audio.ch3.level_control", Labels(AUTO_MANUAL), None),
    p(0xE04B, "audio.ch4.level_control", Labels(AUTO_MANUAL), None),
    p(0xE04C, "audio.ch1.level", Raw, None),
    p(0xE04D, "audio.ch2.level", Raw, None),
    p(0xE04E, "audio.ch3.level", Raw, None),
    p(0xE04F, "audio.ch4.level", Raw, None),
    p(0xE051, "audio.ch1.input", Labels(AUDIO_INPUT), None),
    p(0xE052, "audio.ch2.input", Labels(AUDIO_INPUT), None),
    p(0xE053, "audio.ch3.input", Labels(AUDIO_INPUT), None),
    p(0xE054, "audio.ch4.input", Labels(AUDIO_INPUT), None),
    p(
        0xE050,
        "audio.master_level",
        Raw,
        Some("set_audio_master_level"),
    ),
    // Streaming
    p(0xD119, "streaming.status", Labels(STREAM_STATUS), None),
    p(0xD2BE, "streaming.destination", Raw, None),
    // Pan and tilt
    p(0xE0B6, "pan_tilt.pan", Microdegrees, None),
    p(0xE0B8, "pan_tilt.tilt", Microdegrees, None),
    p(0xE0B7, "pan_tilt.pan_status", Labels(PT_STATUS), None),
    p(0xE0B9, "pan_tilt.tilt_status", Labels(PT_STATUS), None),
    p(0xE0BA, "pan_tilt.pan_limit.min", Microdegrees, None),
    p(0xE0BB, "pan_tilt.pan_limit.max", Microdegrees, None),
    p(0xE0BC, "pan_tilt.tilt_limit.min", Microdegrees, None),
    p(0xE0BD, "pan_tilt.tilt_limit.max", Microdegrees, None),
    p(0xE0BE, "pan_tilt.pan_limit.enabled", OnOff, None),
    p(0xE0BF, "pan_tilt.tilt_limit.enabled", OnOff, None),
    p(0xE0CB, "pan_tilt.preset_slots", Raw, None),
    // Live view
    p(0xD221, "live_view.available", Flag, None),
    p(0xD278, "live_view.url", Raw, None),
    p(
        0xD26A,
        "live_view.quality",
        Labels(LIVE_VIEW_QUALITY),
        Some("set_live_view_quality"),
    ),
    p(
        0xE0CE,
        "live_view.quality_level",
        Raw,
        Some("set_live_view_quality_level"),
    ),
    // Content transfer
    p(0xD295, "content.transfer_ready", Flag, None),
    p(0xD1D4, "content.slot1.list_available", Flag, None),
    p(0xD1D5, "content.slot2.list_available", Flag, None),
    p(0xD1D6, "content.slot1.list_regenerated", Raw, None),
    p(0xD1D7, "content.slot2.list_regenerated", Raw, None),
    p(0xE0D9, "content.slot1.list_updated", Raw, None),
    p(0xE0DA, "content.slot2.list_updated", Raw, None),
    p(0xD031, "media.slot1.profile_url", Raw, None),
    p(0xD032, "media.slot2.profile_url", Raw, None),
    p(0xD191, "media.slot3.profile_url", Raw, None),
    // FTP upload by the camera
    p(0xD041, "ftp.enabled", OnOff, Some("set_ftp_function")),
    p(
        0xD04E,
        "ftp.auto_transfer",
        OnOff,
        Some("set_auto_ftp_transfer"),
    ),
    p(
        0xD04F,
        "ftp.auto_transfer_target",
        Labels(FTP_AUTO_TARGET),
        Some("set_auto_ftp_transfer_target"),
    ),
    p(
        0xD216,
        "ftp.auto_transfer_stills",
        Labels(FTP_STILL_TARGET),
        Some("set_auto_ftp_transfer_stills"),
    ),
    p(
        0xD199,
        "ftp.auto_transfer_movies",
        Labels(FTP_MOVIE_TARGET),
        Some("set_auto_ftp_transfer_movies"),
    ),
    p(
        0xD19A,
        "ftp.transfer_files",
        Labels(FTP_FILE_TARGET),
        Some("set_ftp_transfer_files"),
    ),
    p(
        0xD14B,
        "ftp.transfer_proxy",
        Labels(FTP_PROXY_TARGET),
        Some("set_ftp_transfer_proxy"),
    ),
    p(
        0xD14A,
        "ftp.still_size",
        Labels(FTP_STILL_SIZE),
        Some("set_ftp_still_size"),
    ),
    p(0xD14C, "ftp.power_save", OnOff, Some("set_ftp_power_save")),
    p(
        0xD225,
        "ftp.protect_after_transfer",
        OnOff,
        Some("set_ftp_protect_after_transfer"),
    ),
    p(0xD27C, "ftp.server", Raw, Some("select_ftp_server")),
    p(0xD02E, "ftp.server_id", Raw, Some("select_ftp_server_id")),
    p(0xD27F, "ftp.connection_status", Labels(FTP_STATUS), None),
    p(0xD280, "ftp.connection_error", Labels(FTP_ERROR), None),
    p(0xD09A, "ftp.settings_editable", Flag, None),
    p(0xD02A, "ftp.job_sync_id", Raw, None),
];

/// Properties read by code that has its own commands: tally lamps by colour,
/// audio channels by number, and position targets.
pub(crate) const TALLY: [(&str, u16); 3] = [("red", 0xE0D2), ("green", 0xE0D3), ("yellow", 0xE0D4)];
pub(crate) const AUDIO_LEVEL_CONTROL: [u16; 4] = [0xE048, 0xE049, 0xE04A, 0xE04B];
pub(crate) const AUDIO_LEVEL: [u16; 4] = [0xE04C, 0xE04D, 0xE04E, 0xE04F];
pub(crate) const AUDIO_INPUT_SELECT: [u16; 4] = [0xE051, 0xE052, 0xE053, 0xE054];

pub(crate) fn by_code(code: u16) -> Option<&'static Prop> {
    PROPS.iter().find(|p| p.code == code)
}

pub(crate) fn by_command(command: &str) -> Option<&'static Prop> {
    PROPS.iter().find(|p| p.command == Some(command))
}

pub(crate) fn label(table: &[(i128, &'static str)], v: i128) -> Value {
    match table.iter().find(|(code, _)| *code == v) {
        Some((_, name)) => json!(name),
        None => json!(format!("0x{v:X}")),
    }
}

fn fraction(num: i128, den: i128) -> Value {
    if den == 0 {
        return Value::Null;
    }
    if num == 1 && den > 1 {
        return json!(format!("1/{den}"));
    }
    let seconds = num as f64 / den as f64;
    json!(format!("{}\"", trim_float(seconds)))
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// The state value of a property's wire value.
pub(crate) fn decode(decode: Decode, value: &PtpValue) -> Value {
    let v = match value {
        PtpValue::Int(v) => *v,
        other => return other.to_json(),
    };
    match decode {
        Raw => value.to_json(),
        Labels(table) => label(table, v),
        OnOff => json!(v == 2),
        Flag => json!(v == 1),
        Scaled(div) => json!(round3(v as f64 / div)),
        FNumber => match v {
            0xFFFD => json!("closed"),
            0xFFFE | 0xFFFF => Value::Null,
            _ => json!(v as f64 / 100.0),
        },
        Shutter32 => match v {
            0 => json!("bulb"),
            0xFFFF_FFFF => Value::Null,
            _ => fraction(v >> 16, v & 0xFFFF),
        },
        Shutter64 => match v {
            0 => json!("bulb"),
            v if v == u64::MAX as i128 => Value::Null,
            _ => fraction(v >> 32, v & 0xFFFF_FFFF),
        },
        Iso => {
            if v & 0x00FF_FFFF == 0x00FF_FFFF {
                json!("auto")
            } else {
                json!(v & 0x00FF_FFFF)
            }
        }
        Percent => {
            if v < 0 || v == 0xFF || v == 0xFFFF {
                Value::Null
            } else {
                json!(v)
            }
        }
        Count32 => {
            if v == 0xFFFF_FFFF {
                Value::Null
            } else {
                json!(v)
            }
        }
        Millivolts => {
            if v == 0xFFFF_FFFF {
                Value::Null
            } else {
                json!(round3(v as f64 / 1000.0))
            }
        }
        Microdegrees => json!(v as f64 / 1_000_000.0),
        Timecode => {
            let b = |shift: u32| (v >> shift) & 0xFF;
            json!(format!("{:02}:{:02}:{:02}:{:02}", b(24), b(16), b(8), b(0)))
        }
        UserBits => json!(format!("{:08X}", v & 0xFFFF_FFFF)),
        Ratio64 => {
            if v == u64::MAX as i128 {
                return Value::Null;
            }
            let (num, den) = (v >> 32, v & 0xFFFF_FFFF);
            if den == 0 {
                Value::Null
            } else {
                json!(format!("{num}/{den}"))
            }
        }
        Progress => {
            let (num, den) = (v >> 16, v & 0xFFFF);
            if den == 0 {
                Value::Null
            } else {
                json!((num * 100) / den)
            }
        }
        FrameRate => frame_rate_label(v),
    }
}

fn frame_rate_label(v: i128) -> Value {
    let (base, suffix) = if v >= 0x41 {
        (v - 0x41, "i")
    } else {
        (v - 1, "p")
    };
    match usize::try_from(base).ok().and_then(|i| FRAME_RATES.get(i)) {
        Some(rate) => json!(format!("{rate}{suffix}")),
        None => json!(format!("0x{v:X}")),
    }
}

fn frame_rate_code(label: &str) -> Option<i128> {
    let (rate, base) = match label.strip_suffix('p') {
        Some(r) => (r, 0x01),
        None => (label.strip_suffix('i')?, 0x41),
    };
    FRAME_RATES
        .iter()
        .position(|r| *r == rate)
        .map(|i| base + i as i128)
}

/// Every frame rate label, progressive then interlaced.
#[cfg(test)]
pub(crate) fn frame_rate_labels() -> Vec<String> {
    let mut out: Vec<String> = FRAME_RATES.iter().map(|r| format!("{r}p")).collect();
    out.extend(FRAME_RATES.iter().map(|r| format!("{r}i")));
    out
}

/// Parses `1/50`, `2.5"`, `2.5`, `30"` or `bulb` into numerator and
/// denominator.
fn parse_shutter(text: &str) -> Result<(i128, i128), String> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("bulb") {
        return Ok((0, 0));
    }
    if let Some(den) = t.strip_prefix("1/") {
        let den: i128 = den
            .parse()
            .map_err(|_| format!("'{text}' is not a shutter speed"))?;
        return Ok((1, den));
    }
    let seconds: f64 = t
        .trim_end_matches('"')
        .trim_end_matches('s')
        .parse()
        .map_err(|_| format!("'{text}' is not a shutter speed"))?;
    if seconds.is_nan() || seconds <= 0.0 {
        return Err(format!("'{text}' is not a shutter speed"));
    }
    Ok(((seconds * 10.0).round() as i128, 10))
}

fn number(value: &Value) -> Result<f64, String> {
    value
        .as_f64()
        .ok_or_else(|| format!("{value} is not a number"))
}

/// The wire value for an operator value, the inverse of `decode`.
pub(crate) fn encode(decode: Decode, value: &Value) -> Result<PtpValue, String> {
    let int = |v: i128| Ok(PtpValue::Int(v));
    match decode {
        Raw => match value {
            Value::String(s) => Ok(PtpValue::Str(s.clone())),
            Value::Number(n) => n
                .as_i64()
                .map(|v| PtpValue::Int(v as i128))
                .or_else(|| n.as_u64().map(|v| PtpValue::Int(v as i128)))
                .ok_or_else(|| format!("{n} is not an integer")),
            other => Err(format!("{other} is not a number or string")),
        },
        Labels(table) => {
            let name = value.as_str().ok_or("expected a value name")?;
            table
                .iter()
                .find(|(_, n)| *n == name)
                .map(|(code, _)| PtpValue::Int(*code))
                .ok_or_else(|| format!("'{name}' is not a known value"))
        }
        OnOff => int(if value.as_bool().ok_or("expected true or false")? {
            2
        } else {
            1
        }),
        Flag => int(if value.as_bool().ok_or("expected true or false")? {
            1
        } else {
            0
        }),
        Scaled(div) => int((number(value)? * div).round() as i128),
        FNumber => int((number(value)? * 100.0).round() as i128),
        Microdegrees => int((number(value)? * 1_000_000.0).round() as i128),
        Shutter32 => {
            let (num, den) = parse_shutter(value.as_str().ok_or("expected text such as 1/50")?)?;
            if num > 0xFFFF || den > 0xFFFF {
                return Err("shutter speed out of range".into());
            }
            int((num << 16) | den)
        }
        Shutter64 => {
            let (num, den) = parse_shutter(value.as_str().ok_or("expected text such as 1/50")?)?;
            if num > 0xFFFF_FFFF || den > 0xFFFF_FFFF {
                return Err("shutter speed out of range".into());
            }
            int((num << 32) | den)
        }
        Iso => match value {
            Value::String(s) if s.eq_ignore_ascii_case("auto") => int(0x00FF_FFFF),
            Value::String(s) => s
                .parse::<i128>()
                .map(PtpValue::Int)
                .map_err(|_| format!("'{s}' is not an ISO value")),
            Value::Number(n) => n
                .as_i64()
                .map(|v| PtpValue::Int(v as i128))
                .ok_or_else(|| format!("{n} is not an ISO value")),
            other => Err(format!("{other} is not an ISO value")),
        },
        Timecode => {
            let text = value.as_str().ok_or("expected HH:MM:SS:FF")?;
            let parts: Vec<i128> = text
                .split(':')
                .map(|p| p.parse::<i128>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("'{text}' is not HH:MM:SS:FF"))?;
            match parts.as_slice() {
                [h, m, s, f] if *h < 24 && *m < 60 && *s < 60 && *f < 60 => {
                    int((h << 24) | (m << 16) | (s << 8) | f)
                }
                _ => Err(format!("'{text}' is not HH:MM:SS:FF")),
            }
        }
        UserBits => {
            let text = value.as_str().ok_or("expected eight hex digits")?;
            if text.len() != 8 {
                return Err(format!("'{text}' is not eight hex digits"));
            }
            i128::from_str_radix(text, 16)
                .map(PtpValue::Int)
                .map_err(|_| format!("'{text}' is not eight hex digits"))
        }
        FrameRate => {
            let text = value
                .as_str()
                .ok_or("expected a frame rate such as 59.94p")?;
            frame_rate_code(text)
                .map(PtpValue::Int)
                .ok_or_else(|| format!("'{text}' is not a known frame rate"))
        }
        Percent | Count32 | Millivolts | Ratio64 | Progress => {
            Err("this property is read-only here; use set_property".into())
        }
    }
}

/// The names a `Labels` reading can take, for the spec's enum parameters.
#[cfg(test)]
pub(crate) fn labels_of(decode: Decode) -> Option<Vec<String>> {
    match decode {
        Labels(table) => Some(table.iter().map(|(_, n)| n.to_string()).collect()),
        FrameRate => Some(frame_rate_labels()),
        _ => None,
    }
}

/// The value type of each documented control, which sets how many bytes
/// SDIO_ControlDevice sends.
pub(crate) const CONTROL_TYPES: &[(u16, u16)] = &[
    (0xD2C1, dt::UINT16),
    (0xD2C2, dt::UINT16),
    (0xD2C3, dt::UINT16),
    (0xD2C8, dt::UINT16),
    (0xD2C9, dt::UINT16),
    (0xD2CD, dt::UINT16),
    (0xD2CE, dt::UINT16),
    (0xD2CF, dt::UINT16),
    (0xD2D0, dt::UINT16),
    (0xD2D1, dt::INT16),
    (0xD2D9, dt::UINT16),
    (0xD2DC, dt::UINT32),
    (0xD2DD, dt::INT8),
    (0xD2DF, dt::UINT16),
    (0xD2E0, dt::UINT16),
    (0xD2E1, dt::UINT32),
    (0xD2E2, dt::UINT16),
    (0xD2E3, dt::INT16),
    (0xD2E4, dt::UINT32),
    (0xD2E5, dt::UINT16),
    (0xD2E6, dt::UINT16),
    (0xD2E7, dt::UINT16),
    (0xD2E9, dt::UINT8),
    (0xD2EA, dt::UINT8),
    (0xD2EB, dt::UINT16),
    (0xD2EC, dt::INT16),
    (0xD2ED, dt::INT16),
    (0xD2EE, dt::UINT16),
    (0xD2EF, dt::INT8),
    (0xD2F0, dt::INT16),
    (0xD2F1, dt::UINT16),
    (0xD2F2, dt::UINT16),
    (0xD2F3, dt::UINT16),
    (0xD2F6, dt::UINT16),
    (0xD2F7, dt::UINT16),
    (0xD2F8, dt::UINT16),
    (0xD2F9, dt::UINT16),
    (0xD2FA, dt::UINT16),
    (0xD2FB, dt::UINT16),
    (0xD2FC, dt::UINT16),
    (0xD2FD, dt::UINT16),
    (0xD2FE, dt::UINT16),
    (0xD2FF, dt::UINT16),
    (0xD300, dt::UINT16),
    (0xD301, dt::UINT16),
    (0xD302, dt::UINT16),
    (0xD303, dt::UINT16),
    (0xD304, dt::UINT16),
    (0xD305, dt::UINT16),
    (0xD306, dt::UINT16),
    (0xD307, dt::UINT16),
    (0xD309, dt::UINT32),
    (0xD30A, dt::UINT32),
    (0xD30B, dt::INT32),
    (0xD30C, dt::INT32),
    (0xD30D, dt::UINT16),
    (0xD30E, dt::UINT16),
    (0xD312, dt::UINT16),
    (0xD313, dt::UINT16),
    (0xD314, dt::STR),
    (0xD315, dt::UINT16),
    (0xD316, dt::UINT16),
    (0xF000, dt::INT16),
    (0xF001, dt::UINT16),
    (0xF002, dt::UINT16),
    (0xF003, dt::INT16),
    (0xF004, dt::INT16),
    (0xF00C, dt::UINT16),
    (0xF012, dt::UINT16),
    (0xF015, dt::UINT16),
    // Camera Control PTP 2: step controls for values its properties only
    // report, and its own buttons.
    (0x5007, dt::INT8),
    (0x5010, dt::INT8),
    (0xD200, dt::INT8),
    (0xD20D, dt::INT8),
    (0xD21E, dt::INT8),
    (0xD2C4, dt::UINT16),
    (0xD2C5, dt::UINT16),
    (0xD2C7, dt::UINT16),
    (0xD2CA, dt::UINT16),
    (0xD2CB, dt::UINT16),
    (0xD2CC, dt::UINT16),
    (0xD2D2, dt::UINT16),
    (0xD2D3, dt::UINT16),
    (0xD2D4, dt::UINT16),
    (0xD2D5, dt::UINT16),
    (0xD2D6, dt::UINT16),
    (0xD2D7, dt::UINT16),
    (0xD2D8, dt::UINT16),
];

pub(crate) fn control_type(code: u16) -> Option<u16> {
    CONTROL_TYPES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, t)| *t)
}

/// Values a Camera Control PTP 2 camera reports as properties but changes
/// only by steps, through a control of the same code.
pub(crate) const STEPPED: [(&str, u16); 5] = [
    ("iris", 0x5007),
    ("exposure_compensation", 0x5010),
    ("flash_compensation", 0xD200),
    ("shutter_speed", 0xD20D),
    ("iso", 0xD21E),
];

/// Remote keys for menu navigation, by name.
pub(crate) const REMOTE_KEYS: &[(&str, u16)] = &[
    ("up", 0xD2CD),
    ("down", 0xD2CE),
    ("left", 0xD2CF),
    ("right", 0xD2D0),
    ("up_right", 0xD2FA),
    ("down_right", 0xD2FB),
    ("up_left", 0xD2FC),
    ("down_left", 0xD2FD),
    ("set", 0xD2F9),
    ("menu", 0xD2FF),
    ("back", 0xD2F7),
    ("display", 0xD2F8),
];

/// What a result parameter of Sony's result events means.
pub(crate) fn result_name(v: u32) -> &'static str {
    match v {
        0x00 => "invalid",
        0x01 => "ok",
        0x02 => "failed",
        0x03 => "invalid_parameter",
        0x04 => "camera_status_error",
        0x05 => "canceled",
        0x11 => "character_size_error",
        0x12 => "too_dark",
        0x13 => "time_limit_exceeded",
        0x14 => "too_bright",
        0x15 => "color_temperature_too_high",
        0x16 => "color_temperature_too_low",
        0x17 => "tint_out_of_range",
        0x18 => "too_little_white",
        _ => "unknown",
    }
}

/// Result parameters of events whose third value means "canceled" rather
/// than an invalid parameter (zoom, focus and pan/tilt results).
pub(crate) fn drive_result_name(v: u32) -> &'static str {
    match v {
        0 => "invalid",
        1 => "ok",
        2 => "failed",
        3 => "canceled",
        _ => "unknown",
    }
}

/// A PTP or Sony response code, in words.
pub(crate) fn response_name(code: u16) -> String {
    match code {
        0x2001 => "ok".into(),
        0x2002 => "general error".into(),
        0x2003 => "session not open".into(),
        0x2004 => "invalid transaction id".into(),
        0x2005 => "operation not supported".into(),
        0x2006 => "parameter not supported".into(),
        0x2007 => "incomplete transfer".into(),
        0x2019 => "device busy".into(),
        0x201D => "invalid parameter".into(),
        0x201E => "session already open".into(),
        0xA101 => "authentication failed".into(),
        0xA102 => "password too long".into(),
        0xA103 => "password has an invalid character".into(),
        0xA104 => "invalid feature version".into(),
        0xA105 => "temporary storage full".into(),
        0xA106 => "camera status error".into(),
        other => format!("response 0x{other:04X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(v: i128) -> PtpValue {
        PtpValue::Int(v)
    }

    #[test]
    fn codes_and_paths_are_unique_and_commands_named_once() {
        let mut codes = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        let mut commands = std::collections::HashSet::new();
        for prop in PROPS {
            assert!(codes.insert(prop.code), "0x{:04X} twice", prop.code);
            assert!(paths.insert(prop.path), "{} twice", prop.path);
            if let Some(c) = prop.command {
                assert!(commands.insert(c), "{c} twice");
            }
        }
        // No path is the parent of another.
        for a in &paths {
            for b in &paths {
                assert!(!b.starts_with(&format!("{a}.")), "{a} contains {b}");
            }
        }
    }

    #[test]
    fn exposure_readings() {
        assert_eq!(decode(FNumber, &int(450)), json!(4.5));
        assert_eq!(decode(FNumber, &int(0xFFFD)), json!("closed"));
        assert_eq!(decode(FNumber, &int(0xFFFF)), Value::Null);
        assert_eq!(decode(Shutter32, &int(0x0001_0032)), json!("1/50"));
        assert_eq!(decode(Shutter32, &int(0x000F_000A)), json!("1.5\""));
        assert_eq!(decode(Shutter32, &int(0x012C_000A)), json!("30\""));
        assert_eq!(decode(Shutter32, &int(0)), json!("bulb"));
        assert_eq!(decode(Shutter64, &int((1 << 32) | 48)), json!("1/48"));
        assert_eq!(decode(Iso, &int(0x00FF_FFFF)), json!("auto"));
        assert_eq!(decode(Iso, &int(0x0100_0320)), json!(800));
        assert_eq!(decode(Scaled(1000.0), &int(-1300)), json!(-1.3));
        assert_eq!(
            decode(Labels(EXPOSURE_MODE), &int(0x0007_8053)),
            json!("movie_manual")
        );
        assert_eq!(decode(Labels(EXPOSURE_MODE), &int(0x1234)), json!("0x1234"));
    }

    #[test]
    fn readings_round_trip_through_their_commands() {
        let cases: Vec<(Decode, Value, i128)> = vec![
            (FNumber, json!(2.8), 280),
            (Shutter32, json!("1/50"), 0x0001_0032),
            (Shutter32, json!("1.5\""), 0x000F_000A),
            (Shutter64, json!("1/48"), (1 << 32) | 48),
            (Iso, json!("auto"), 0x00FF_FFFF),
            (Iso, json!("800"), 800),
            (Scaled(1000.0), json!(-1.3), -1300),
            (Scaled(10.0), json!(12.5), 125),
            (OnOff, json!(true), 2),
            (Flag, json!(false), 0),
            (Labels(WHITE_BALANCE), json!("daylight"), 4),
            (Timecode, json!("01:02:03:04"), 0x0102_0304),
            (UserBits, json!("00FF10AB"), 0x00FF_10AB),
            (FrameRate, json!("59.94p"), 0x0A),
            (FrameRate, json!("50i"), 0x44),
            (Microdegrees, json!(-12.5), -12_500_000),
        ];
        for (d, operator, wire) in cases {
            assert_eq!(encode(d, &operator).unwrap(), int(wire), "{operator}");
            let back = decode(d, &int(wire));
            match d {
                // Text input forms that read back in the camera's style.
                Iso => assert!(back == operator || back == json!(800)),
                _ => assert_eq!(back, operator, "0x{wire:X}"),
            }
        }
    }

    #[test]
    fn bad_operator_values_are_refused() {
        assert!(encode(Labels(WHITE_BALANCE), &json!("purple")).is_err());
        assert!(encode(Shutter32, &json!("fast")).is_err());
        assert!(encode(Timecode, &json!("25:00:00:00")).is_err());
        assert!(encode(UserBits, &json!("12")).is_err());
        assert!(encode(Percent, &json!(5)).is_err());
    }

    #[test]
    fn power_and_media_readings() {
        assert_eq!(decode(Percent, &int(-1)), Value::Null);
        assert_eq!(decode(Percent, &int(80)), json!(80));
        assert_eq!(decode(Millivolts, &int(7_400)), json!(7.4));
        assert_eq!(decode(Count32, &int(0xFFFF_FFFF)), Value::Null);
        assert_eq!(decode(Progress, &int(0x0036_00C8)), json!(27));
        assert_eq!(decode(Ratio64, &int((1 << 32) | 16)), json!("1/16"));
        assert_eq!(decode(Labels(MEDIA_STATUS), &int(2)), json!("no_card"));
    }

    #[test]
    fn every_documented_control_has_a_type() {
        for (_, code) in REMOTE_KEYS {
            assert!(control_type(*code).is_some());
        }
        assert_eq!(control_type(0xF003), Some(dt::INT16));
        assert_eq!(control_type(0xD2DD), Some(dt::INT8));
        assert_eq!(control_type(0x1234), None);
    }
}
