"""Generate the Samsung MDC spec and the native module's command table from one
table, since every MDC command is a byte, an optional sub-command and a fixed
layout of fields.

    python tools/generate_samsung_mdc.py

Writes specs/samsung-mdc.yaml and crates/core/src/modules/samsung_mdc_table.rs.
Edit this file, not its output: tools/test_generate_samsung_mdc.py fails when
the committed files differ from what it writes.

Wire layouts are restated from Samsung's "Multiple Display Control Protocol"
Ver. 15.0 (2020-11-06); each command names its section and page there.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent

# ── Value tables ────────────────────────────────────────────────────────────
# (code, value as the operator names it, settable). A code marked False is
# only ever reported by the display (§2.1.14 marks DVI_VIDEO and the HDMIx_PC
# codes get only).

INPUTS = [
    (0x04, "s_video", True), (0x08, "component", True), (0x0C, "av1", True), (0x0D, "av2", True),
    (0x0E, "scart1", True), (0x14, "pc", True), (0x18, "dvi", True), (0x1E, "bnc", True),
    (0x1F, "dvi_video", False), (0x20, "magicinfo", True), (0x21, "hdmi1", True),
    (0x22, "hdmi1_pc", False), (0x23, "hdmi2", True), (0x24, "hdmi2_pc", False),
    (0x25, "displayport1", True), (0x26, "displayport2", True), (0x27, "displayport3", True),
    (0x30, "rf_tv", True), (0x31, "hdmi3", True), (0x32, "hdmi3_pc", False), (0x33, "hdmi4", True),
    (0x34, "hdmi4_pc", False), (0x40, "dtv", True), (0x50, "plug_in_module", True),
    (0x55, "hdbaset", True), (0x56, "ocm", True), (0x60, "media_magicinfo_s", True),
    (0x61, "screen_mirroring", True), (0x62, "internal_usb", True), (0x63, "url_launcher", True),
    (0x64, "iwb", False), (0x65, "web_browser", True), (0x66, "remote_workspace", True),
]
INPUTS_DESC = ("one of the §2.1.14 input codes: s_video, component, av1, av2, scart1, pc, dvi, bnc, magicinfo, "
               "hdmi1-hdmi4, displayport1-displayport3, rf_tv, dtv, plug_in_module, hdbaset, ocm, media_magicinfo_s, "
               "screen_mirroring, internal_usb, url_launcher, web_browser or remote_workspace (dvi_video, hdmiN_pc and "
               "iwb are only reported)")


def onoff(off="off", on="on"):
    return [(0x00, off, True), (0x01, on, True)]


ONOFF = onoff()
AMPM = [(0x00, "pm", True), (0x01, "am", True)]
ORIENTATION = [(0x00, "landscape", True), (0x01, "portrait", True), (0x02, "landscape_flipped", True),
               (0x03, "portrait_flipped", True)]
POWER = [(0x00, "off", True), (0x01, "on", True), (0x02, "reboot", True)]
PICTURE_SIZE = [
    (0x00, "auto_wide", True), (0x01, "16_9", True), (0x04, "zoom", True), (0x05, "zoom1", True),
    (0x06, "zoom2", True), (0x09, "screen_fit", True), (0x0B, "4_3", True), (0x0C, "wide_fit", True),
    (0x0D, "custom", True), (0x0E, "smart_view1", True), (0x0F, "smart_view2", True),
    (0x10, "pc_16_9", True), (0x18, "pc_4_3", True), (0x20, "pc_original_ratio", True),
    (0x21, "pc_21_9", True), (0x22, "pc_custom", True), (0x31, "wide_zoom", True), (0x32, "21_9", True),
]
COLOR_TONE = [(0x00, "cool2", True), (0x01, "cool1", True), (0x02, "normal", True), (0x03, "warm1", True),
              (0x04, "warm2", True), (0x05, "natural", True), (0x50, "off", True)]
PC_COLOR_TONE = [(0x00, "custom", True), (0x01, "cool", True), (0x02, "normal", True), (0x03, "warm", True),
                 (0x05, "natural", True), (0x50, "off", True)]
# §2.1.3F: C_Temp Extended codes are the kelvin / 100; the older C_Temp set
# uses 0x00-0x0A (5000 K to 15000 K, steps not tabulated) and 0xFD-0xFF.
COLOR_TEMP = ([(k // 100, f"{k}k", True) for k in range(2800, 16001, 500) if k != 2900]
              + [(i, f"legacy_{i}", True) for i in range(0x00, 0x0B)]
              + [(0xFD, "legacy_2800k", True), (0xFE, "legacy_3000k", True), (0xFF, "legacy_4000k", True)])
COLOR_TEMP = sorted({c: (c, v, s) for c, v, s in COLOR_TEMP}.values())
PIP_SIZE = [(0x00, "off", True), (0x04, "double_window", True), (0x05, "double_wide", True),
            (0x06, "medium", True), (0x07, "large", True), (0x08, "small", True), (0x09, "pop", True),
            (0x10, "custom", True)]
SAFETY_SCREEN_TYPES = [
    (0x00, "off", True), (0x03, "scroll", True), (0x04, "pixel", True), (0x05, "bar", True),
    (0x06, "eraser", True), (0x09, "all_white", True), (0x0A, "pattern", True), (0x10, "rolling_bar", True),
    (0x11, "fading_screen", True), (0x83, "scroll_interval", True), (0x84, "pixel_interval", True),
    (0x85, "bar_interval", True), (0x86, "eraser_interval", True), (0x89, "all_white_interval", True),
    (0x8A, "pattern_interval", True), (0x90, "rolling_bar_interval", True),
    (0x91, "fading_screen_interval", True),
]
TIMER_REPEAT = [(0x00, "once", True), (0x01, "everyday", True), (0x02, "mon_fri", True),
                (0x03, "mon_sat", True), (0x04, "sat_sun", True), (0x05, "manual", True)]
HOLIDAY_APPLY = [(0x00, "none", True), (0x01, "both", True), (0x02, "on_timer_only", True),
                 (0x03, "off_timer_only", True)]
WALL_DIV = [(0x00, "off", True)] + [((h << 4) | v, f"{h}x{v}", True) for h in range(1, 16) for v in range(1, 16)]
PITCH = [(0x01, "1.15", True), (0x02, "1.5", True), (0x03, "2.0", True), (0x04, "2.5", True),
         (0x05, "3.0", True), (0x06, "4.0", True), (0x07, "6.0", True), (0x08, "8.0", True),
         (0x09, "0.84", True), (0x0A, "1.26", True), (0x0B, "1.68", True), (0x0C, "2.1", True),
         (0x0D, "8.5", True), (0x0E, "0.51", True), (0x0F, "0.63", True)]
LED_PHY_SIZE = [(0x01, "368x414", True), (0x02, "480x540", True), (0x03, "480x720", True),
                (0x04, "960x540", True), (0x05, "806x453", True), (0x06, "403x453", True),
                (0x07, "240x540", True), (0x08, "2422x1364", True)]
LED_ASPECT = [(0x01, "8:9", True), (0x02, "2:3", True), (0x03, "16:9", True), (0x04, "8:9_half", True),
              (0x05, "4:9", True)]
LED_MODULES = [(0x01, "2x3", True), (0x02, "2x4", True), (0x03, "4x3", True), (0x04, "8x3", True),
               (0x05, "1x3", True), (0x06, "8x6", True)]
GAMUT = [(0x00, "natural", True), (0x01, "srgb", True), (0x02, "adobe_rgb", True), (0x03, "dci_p3", True)]
SW_DATA_TYPES = [(0x00, "cabinet_main_fw", True), (0x01, "cabinet_fpga_fw", True),
                 (0x02, "cabinet_calibration", True), (0x03, "sbox_fpga_fw", True),
                 (0x04, "sbox_valens_fw", True), (0x05, "ig_valens_fw", True),
                 (0x06, "sbox_mbox_fpga2_fw", True), (0x07, "cabinet_peripheral1", True)] + [
                    (0x07 + i, f"cabinet_peripheral{i + 1}", True) for i in range(1, 10)]
KEYS = [
    (0x01, "source", True), (0x02, "power", True), (0x04, "1", True), (0x05, "2", True), (0x06, "3", True),
    (0x07, "volume_up", True), (0x08, "4", True), (0x09, "5", True), (0x0A, "6", True),
    (0x0B, "volume_down", True), (0x0C, "7", True), (0x0D, "8", True), (0x0E, "9", True),
    (0x0F, "mute", True), (0x10, "channel_down", True), (0x11, "0", True), (0x12, "channel_up", True),
    (0x14, "green", True), (0x15, "yellow", True), (0x16, "cyan", True), (0x1A, "menu", True),
    (0x1F, "display", True), (0x23, "digit", True), (0x24, "pip_tv_video", True), (0x2D, "exit", True),
    (0x30, "magicinfo", True), (0x45, "rewind", True), (0x46, "stop", True), (0x47, "play", True),
    (0x48, "fast_forward", True), (0x4A, "pause", True), (0x4B, "tools", True), (0x58, "return", True),
    (0x5B, "magicinfo_lite", True), (0x60, "up", True), (0x61, "down", True), (0x62, "right", True),
    (0x65, "left", True), (0x68, "enter", True), (0x6C, "red", True), (0x77, "lock", True),
    (0x79, "content", True), (0x98, "power_off", True), (0x9F, "3d", True),
]
PMODE = [
    (0x00, "dynamic", True), (0x01, "standard", True), (0x02, "movie", True), (0x03, "custom", True),
    (0x04, "natural", True), (0x05, "calibration", True), (0x10, "entertain", True),
    (0x11, "internet", True), (0x12, "text", True), (0x13, "pc_custom", True),
    (0x14, "advertisement", True), (0x15, "information", True), (0x16, "pc_calibration", True),
    (0x20, "shop_mall_video", True), (0x21, "shop_mall_text", True),
    (0x22, "office_school_video", True), (0x23, "office_school_text", True),
    (0x24, "terminal_station_video", True), (0x25, "terminal_station_text", True),
    (0x26, "videowall_video", True), (0x27, "videowall_text", True), (0x30, "hdr_plus", False),
    (0x50, "off", True),
]
GAMMA = [(0x00, "natural", True), (0x01, "mode1", True), (0x02, "mode2", True), (0x03, "mode3", True),
         (0x04, "mode4", True), (0x05, "mode5", True), (0x11, "minus1", True), (0x12, "minus2", True),
         (0x13, "minus3", True), (0x14, "minus4", True), (0x15, "minus5", True), (0x20, "custom", True)]
SPLIT_PICTURE = [(0x09, "full", True), (0x20, "original", True)]
SPLIT_ON = [(0x00, "off", True), (0x01, "3_screen", True), (0x02, "4_screen", True)]
SPLIT_SOUND = [(0x00, "main", True), (0x01, "sub1", True), (0x02, "sub2", True), (0x03, "sub3", True)]
SPLIT_SIZE = [(0x00, "mode1", True), (0x01, "mode2", True), (0x02, "mode3", True), (0x03, "mode4", True),
              (0x04, "mode5", True), (0x05, "mode6", True)]

TABLES = {}   # rust const name -> table, filled as fields are made


def table_name(values):
    for name, t in TABLES.items():
        if t == values:
            return name
    name = f"T{len(TABLES)}"
    TABLES[name] = values
    return name


# ── Fields ──────────────────────────────────────────────────────────────────

class F:
    """One field of a frame's data. kind is one of: u8, u16, i8, i16, enum,
    flags, text, fixed, ptext, ip4, hex, lit, skip, list."""

    def __init__(self, name, kind, desc="", *, lo=None, hi=None, values=None, unit=None, label=None,
                 n=None, item=None, prefix=False, value=None, bits=None):
        self.name, self.kind, self.desc, self.lo, self.hi = name, kind, desc, lo, hi
        self.values, self.unit, self.label, self.n = values, unit, label, n
        self.item, self.prefix, self.value, self.bits = item, prefix, value, bits

    @property
    def is_value(self):
        return self.kind not in ("lit", "skip")


def U8(name, lo, hi, desc, **k):
    return F(name, "u8", desc, lo=lo, hi=hi, **k)


def U16(name, lo, hi, desc, **k):
    return F(name, "u16", desc, lo=lo, hi=hi, **k)


def I8(name, lo, hi, desc, **k):
    return F(name, "i8", desc, lo=lo, hi=hi, **k)


def E(name, values, desc, **k):
    return F(name, "enum", desc, values=values, **k)


def ON(name, desc, off="off", on="on", **k):
    return E(name, onoff(off, on), desc, **k)


def TEXT(name, n, desc, **k):
    return F(name, "text", desc, n=n, **k)


def LIT(v):
    return F("_", "lit", value=v)


def SKIP(n):
    return F("_", "skip", n=n)


def LIST(name, item, desc, prefix=False):
    return F(name, "list", desc, item=item, prefix=prefix)


def FLAGS(name, bits, desc):
    return F(name, "flags", desc, bits=bits)


def HOUR12(prefix, what):
    return [U8(f"{prefix}hour", 1, 12, f"Hour of the {what}, 1-12 on a 12-hour clock with the AM/PM value beside it."),
            U8(f"{prefix}minute", 0, 59, f"Minute of the {what}, 0-59."),
            E(f"{prefix}ampm", AMPM, f"Whether the {what}'s hour is AM or PM.", label=f"{label_of(prefix + 'ampm')}")]


def label_of(name):
    s = name.replace("_", " ").strip()
    s = re.sub(r"\bampm\b", "AM/PM", s)
    s = re.sub(r"\bpip\b", "PIP", s)
    s = re.sub(r"\bhdmi\b", "HDMI", s)
    s = re.sub(r"\bip\b", "IP", s)
    s = re.sub(r"\bdns\b", "DNS", s)
    s = re.sub(r"\burl\b", "URL", s)
    s = re.sub(r"\bhdr\b", "HDR", s)
    s = re.sub(r"\bosd\b", "OSD", s)
    s = re.sub(r"\bid\b", "ID", s)
    s = re.sub(r"\bwb\b", "white balance", s)
    s = re.sub(r"\beq\b", "EQ", s)
    s = re.sub(r"\bcec\b", "CEC", s)
    return s[0].upper() + s[1:]


# ── Commands ────────────────────────────────────────────────────────────────

DEFS = []


class D:
    def __init__(self, key, cmd, sub, section, page, summary, *, get=True, set=None, ack=None, set_name=None,
                 set_summary=None, get_summary=None, poll="slow", critical=None, set_ack=None, flat=False,
                 get_args=None, led=False, state_desc=None, get_name=None, state=""):
        self.key, self.cmd, self.sub, self.section, self.page = key, cmd, sub, section, page
        self.summary, self.get, self.set, self.set_name = summary, get, set, set_name
        self.set_summary, self.get_summary, self.poll, self.critical = set_summary, get_summary, poll, critical
        self.ack = ack if ack is not None else ([set] if set is not None else None)
        self.set_ack, self.flat, self.get_args, self.led = set_ack, flat, get_args or [], led
        self.state_desc, self.get_name = state_desc, get_name
        # The state key replies write: the key for anything read back, a
        # sibling's for a second form of its set, none for an action.
        self.state = key if state == "" and get else (state or None)
        DEFS.append(self)


def simple(key, cmd, sub, section, page, summary, field, *, get=True, set=True, **k):
    return D(key, cmd, sub, section, page, summary, get=get, set=[field] if set else None, ack=[[field]], **k)


# Status reads that fill the same state as the single-value commands (flat).
D("status", 0x00, None, "2.1.00", 28, "the power, volume, mute, input and picture size in one reply", flat=True, poll="fast",
  ack=[[E("power", POWER, "Power state."), U8("volume", 0, 100, "Volume, 0-100; absent on a display without audio."),
        ON("mute", "Whether the sound is muted; absent on a display without audio."),
        E("input", INPUTS, f"The input shown: {INPUTS_DESC}."),
        E("picture_size", PICTURE_SIZE, "Picture size (aspect ratio), as set_picture_size names it."),
        SKIP(2)]])
D("video", 0x04, None, "2.1.04", 29, "the video picture settings in one reply", flat=True,
  ack=[[U8("contrast", 0, 100, "Contrast, 0-100."), U8("brightness", 0, 100, "Brightness, 0-100."),
        U8("sharpness", 0, 100, "Sharpness, 0-100."), U8("color", 0, 100, "Color saturation, 0-100."),
        U8("tint", 0, 100, "Tint, 0-100: the red share, green being 100 minus it."),
        E("color_tone", COLOR_TONE, "Color tone."), E("color_temperature", COLOR_TEMP, "Color temperature."),
        SKIP(1)]])
D("rgb", 0x06, None, "2.1.06", 30, "the PC-source picture settings and RGB gains in one reply",
  ack=[[U8("contrast", 0, 100, "PC-source contrast, 0-100."), U8("brightness", 0, 100, "PC-source brightness, 0-100."),
        E("color_tone", COLOR_TONE, "PC-source color tone."), E("color_temperature", COLOR_TEMP, "PC-source color temperature."),
        SKIP(1), U8("red_gain", 0, 254, "Red gain; absent on a display without RGB gains."),
        U8("green_gain", 0, 254, "Green gain; absent on a display without RGB gains."),
        U8("blue_gain", 0, 254, "Blue gain; absent on a display without RGB gains.")]])
D("pip_status", 0x07, None, "2.1.07", 31, "the PIP size and source",
  ack=[[E("size", PIP_SIZE, "PIP size, as set_pip_size names it."), E("source", INPUTS, "PIP source input."), SKIP(2)]])
_lamp = [U8("lamp_max_hour", 1, 12, "Hour the automatic lamp's maximum applies, 1-12."),
         U8("lamp_max_minute", 0, 59, "Minute the automatic lamp's maximum applies."),
         E("lamp_max_ampm", AMPM, "AM or PM for the automatic lamp's maximum."),
         U8("lamp_max_value", 0, 100, "Automatic lamp maximum, 0-100."),
         U8("lamp_min_hour", 1, 12, "Hour the automatic lamp's minimum applies, 1-12."),
         U8("lamp_min_minute", 0, 59, "Minute the automatic lamp's minimum applies."),
         E("lamp_min_ampm", AMPM, "AM or PM for the automatic lamp's minimum."),
         U8("lamp_min_value", 0, 100, "Automatic lamp minimum, 0-100; absent while automatic lamp control is off."),
         U8("lamp_value", 0, 100, "Manual lamp value, 0-100; absent while manual lamp control is off.")]
_wall = [ON("video_wall", "Whether the video wall is on; absent on a display without video wall."),
         U8("video_wall_format", 0, 254, "Video wall format code."),
         E("video_wall_division", WALL_DIV, "Video wall division, HxV such as 2x2, or off."),
         U8("video_wall_position", 0, 254, "This display's position number in the wall.")]
D("maintenance", 0x08, None, "2.1.08", 32, "the maintenance status: power, PIP, lamp schedule, safety screen and video wall",
  ack=[[E("power", POWER, "Power state."), E("pip_size", PIP_SIZE, "PIP size."), E("pip_source", INPUTS, "PIP source."),
        *_lamp, U8("safety_screen_interval", 0, 10, "Safety screen interval in hours; 0 is off.", unit="h"),
        U8("safety_screen_time", 0, 5, "Safety screen time code; 0 is off."),
        E("safety_screen_type", SAFETY_SCREEN_TYPES, "Safety screen type."), *_wall],
       [E("power", POWER, "Power state."), E("pip_size", PIP_SIZE, "PIP size."), E("pip_source", INPUTS, "PIP source."),
        *_lamp, U8("safety_screen_start_hour", 1, 12, "Safety screen start hour, 1-12."),
        U8("safety_screen_start_minute", 0, 59, "Safety screen start minute."),
        E("safety_screen_type", SAFETY_SCREEN_TYPES, "Safety screen type."),
        E("safety_screen_start_ampm", AMPM, "AM or PM for the safety screen start."),
        U8("safety_screen_end_hour", 1, 12, "Safety screen end hour, 1-12."),
        U8("safety_screen_end_minute", 0, 59, "Safety screen end minute."),
        E("safety_screen_end_ampm", AMPM, "AM or PM for the safety screen end."), *_wall]])
D("sound", 0x09, None, "2.1.09", 35, "the sound settings in one reply",
  ack=[[U8("volume", 0, 100, "Volume, 0-100."), U8("balance", 0, 254, "Balance code."),
        U8("eq_100hz", 0, 20, "100 Hz equalizer, 0-20 for -10 to +10."), U8("eq_300hz", 0, 20, "300 Hz equalizer, 0-20."),
        U8("eq_1khz", 0, 20, "1 kHz equalizer, 0-20."), U8("eq_3khz", 0, 20, "3 kHz equalizer, 0-20."),
        U8("eq_10khz", 0, 20, "10 kHz equalizer, 0-20."), SKIP(3), ON("srs", "Whether SRS TruSurround XT is on.")],
       [U8("volume", 0, 100, "Volume, 0-100."), U8("balance", 0, 254, "Balance code."),
        U8("eq_100hz", 0, 20, "100 Hz equalizer, 0-20."), U8("eq_200hz", 0, 20, "200 Hz equalizer, 0-20."),
        U8("eq_500hz", 0, 20, "500 Hz equalizer, 0-20."), U8("eq_1khz", 0, 20, "1 kHz equalizer, 0-20."),
        U8("eq_2khz", 0, 20, "2 kHz equalizer, 0-20."), U8("eq_5khz", 0, 20, "5 kHz equalizer, 0-20."),
        U8("eq_10khz", 0, 20, "10 kHz equalizer, 0-20."), ON("srs", "Whether SRS TruSurround XT is on.")]])
D("serial_number", 0x0B, None, "2.1.0B", 36, "the serial number", poll="identity",
  ack=[[F("serial_number", "fixed", "Serial number.", n=15), SKIP(3)], [TEXT("serial_number", 255, "Serial number.")]])
D("display_status", 0x0D, None, "2.1.0D", 37, "the error and temperature status", poll="fast",
  ack=[[E("lamp", [(0, "normal", True), (1, "error", True)], "Lamp status."),
        E("temperature", [(0, "normal", True), (1, "error", True)], "Temperature status."),
        E("brightness_sensor", [(0, "none", True), (1, "error", True), (2, "normal", True)], "Brightness sensor status."),
        E("sync", [(0, "normal", True), (1, "no_sync", True), (2, "invalid", True)],
          "Input signal status: no_sync when there is no signal or it is not supported, invalid on an app source."),
        I8("temperature_c", -60, 125, "Internal temperature in degrees Celsius.", unit="°C"),
        E("fan", [(0, "normal", True), (1, "error", True), (2, "not_supported", True)], "Fan status.")]])
D("software_version", 0x0E, None, "2.1.0E", 38, "the software version", poll="identity",
  ack=[[TEXT("software_version", 50, "Software version text: project information, then the version.")]])
D("auto_motion_plus", 0x0F, None, "2.1.0F", 39, "Auto Motion Plus",
  set=[E("mode", [(0, "off", True), (1, "clear", True), (2, "standard", True), (3, "smooth", True), (4, "custom", True),
                  (5, "demo", True), (6, "auto", True)], "Auto Motion Plus mode."),
       U8("blur_reduction", 0, 10, "Blur reduction, 0-10; used in the custom mode."),
       U8("judder_reduction", 0, 10, "Judder reduction, 0-10; used in the custom mode.")])
D("model", 0x10, None, "2.1.10", 40, "the panel type and model code", poll="identity",
  ack=[[E("panel", [(1, "pdp", True), (2, "lcd", True), (3, "dlp", True), (4, "led", True), (5, "crt", True),
                    (6, "oled", True)], "Panel type."),
        U8("code", 0, 255, "Model code from the document's model table (§2.1.10)."),
        E("tv", [(0, "supported", True), (1, "not_supported", True)], "Whether the display has a TV tuner.")]])
simple("power", 0x11, None, "2.1.11", 45, "the power state", E("power", POWER, "on, off, or reboot to restart the display."),
       critical="reboot")
simple("volume", 0x12, None, "2.1.12", 47, "the volume", U8("volume", 0, 100, "Volume, 0-100."))
simple("mute", 0x13, None, "2.1.13", 48, "sound mute", ON("mute", "On mutes the sound, off unmutes it.", label="Mute"))
simple("input", 0x14, None, "2.1.14", 49, "the input source", E("input", INPUTS, f"The input to show: {INPUTS_DESC}."))
simple("picture_size", 0x15, None, "2.1.15", 51, "the picture size (aspect ratio)",
       E("size", PICTURE_SIZE, "Picture size: video-source sizes (auto_wide, 16_9, zoom, ...) or the pc_ sizes for PC sources."))
D("tv_channel", 0x17, None, "2.1.17", 53, "the tuner channel",
  set=[E("country", [(0, "korea", True), (1, "usa", True)], "Channel table country; the document lists only these two."),
       E("band", [(0, "analog", True), (1, "digital", True)], "Analog or digital TV."),
       E("source", [(0, "air", True), (1, "cable", True)], "Air (antenna) or cable."),
       U16("channel", 0, 999, "Major channel: 1-135 analog, 0-999 digital."),
       ON("minor_selected", "On tunes the minor channel too (digital only)."),
       U16("minor_channel", 0, 999, "Minor channel, 0-999 (digital only).")])
simple("screen_mode", 0x18, None, "2.1.18", 54, "the screen mode (tuner aspect)",
       E("mode", [(1, "16_9", True), (4, "zoom", True), (0x0B, "4_3", True), (0x31, "wide_zoom", True)], "Screen mode."))
simple("screen_size", 0x19, None, "2.1.19", 55, "the screen size in inches", U8("inches", 0, 255, "Screen diagonal in inches.", unit="in"),
       set=False, poll="identity")
simple("outdoor_mode", 0x1A, 0x81, "2.1.1A.81", 57, "outdoor mode", ON("mode", "On selects the outdoor mode."))
simple("heatex_fan_speed", 0x1A, 0x82, "2.1.1A.82", 58, "the internal heat exchanger fan speed",
       U8("speed", 0, 100, "Fan speed, 0-100."))
D("network", 0x1B, 0x82, "2.1.1B.82", 60, "the IP address, subnet mask, gateway and DNS server",
  set=[F("ip_address", "ip4", "IPv4 address, such as 192.168.0.10."), F("subnet_mask", "ip4", "Subnet mask, such as 255.255.255.0."),
       F("gateway", "ip4", "Default gateway address."), F("dns_server", "ip4", "DNS server address.")],
  critical="moves the display to another address; a network connection is then lost")
simple("ip_mode", 0x1B, 0x85, "2.1.1B.85", 62, "dynamic (DHCP) or static addressing",
       E("mode", [(0, "dynamic", True), (1, "static", True)], "dynamic takes an address by DHCP, static uses set_network's."),
       critical="can move the display to another address")
D("wifi_access_point", 0x1B, 0x8A, "2.1.1B.8A", 63, "the Wi-Fi network the display joins", get=False,
  set=[LIT(0x00), F("ssid", "ptext", "Network name (SSID), up to 32 characters.", n=32),
       LIT(0x01), F("password", "ptext", "Network password, sent in clear text.", n=64)],
  ack=[[F("data", "hex", "The display's answer, as hex.")]], set_name="set_wifi_access_point",
  critical="changes the network the display is on")
D("weekly_restart", 0x1B, 0xA2, "2.1.1B.A2", 64, "the weekly restart schedule",
  set=[U8("days", 0, 127, "Days to restart as a bit mask: 64 Monday, 32 Tuesday, 16 Wednesday, 8 Thursday, 4 Friday, 2 Saturday, 1 Sunday; 0 never."),
       U8("hour", 0, 23, "Restart hour, 0-23."), U8("minute", 0, 59, "Restart minute, 0-59.")])
D("software_versions", 0x1B, 0xA4, "2.1.1B.A4", 65, "the versions of each LED cabinet and box component", led=True, poll="identity",
  ack=[[LIST("versions", [E("component", SW_DATA_TYPES, "Component."), F("version", "ptext", "Version.", n=255)],
             "Each component's version, {component, version}.", prefix=True)]])
simple("magicinfo_channel", 0x1C, 0x81, "2.1.1C.81", 68, "the MagicInfo channel", U16("channel", 0, 65535, "MagicInfo channel number."),
       get=False)
simple("magicinfo_server", 0x1C, 0x82, "2.1.1C.82", 69, "the MagicInfo server address",
       TEXT("url", 252, "MagicInfo server address, such as http://10.88.8.73:7001, up to 252 characters."))
simple("magicinfo_orientation", 0x1C, 0x83, "2.1.1C.83", 70, "the MagicInfo content orientation",
       E("orientation", ORIENTATION, "Content orientation: landscape, portrait (270°), landscape_flipped (180°) or portrait_flipped (90°)."))
simple("mdc_connection", 0x1D, None, "2.1.1D", 71, "whether MDC is taken over RS-232C or RJ45",
       E("connection", [(0, "rs232c", True), (1, "rj45", True)], "The connection MDC commands are taken on."),
       critical="switching MDC to the other connection can stop this one from answering")
simple("still", 0x1F, None, "2.1.1F", 72, "picture freeze", ON("still", "On freezes the picture of an external input.", label="Freeze"))
simple("led_picture_size", 0x21, 0x01, "2.1.21.01", 74, "the LED picture size", E("size", [(0, "original", True), (1, "custom", True)], "Picture size."), led=True)
D("custom_fit_size", 0x21, 0x02, "2.1.21.02", 75, "the custom picture size",
  set=[U16("width", 0, 65535, "Width in pixels."), U16("height", 0, 65535, "Height in pixels.")])
simple("inverse_tone_mapping", 0x21, 0x03, "2.1.21.03", 76, "HDR inverse tone mapping", ON("enabled", "On turns inverse tone mapping on."))
simple("dynamic_peaking", 0x21, 0x04, "2.1.21.04", 77, "HDR dynamic peaking", ON("enabled", "On turns dynamic peaking on."))
simple("color_mapping", 0x21, 0x05, "2.1.21.05", 78, "HDR color mapping", ON("enabled", "On turns color mapping on."))
simple("fit_to_screen_size", 0x21, 0x06, "2.1.21.06", 79, "picture size fit to screen", ON("enabled", "On fits the picture to the screen."))
D("hdmi_uhd_color", 0x21, 0x07, "2.1.21.07", 80, "HDMI UHD color per input",
  set=[E("input", INPUTS, "The input to change."), ON("enabled", "On turns UHD color on for that input.")],
  ack=[[LIST("inputs", [E("input", INPUTS, "Input."), ON("enabled", "UHD color on.")], "Each input's UHD color, {input, enabled}.")]],
  critical="the display restarts on its own when the value changes")
simple("uhd_output", 0x21, 0x08, "2.1.21.08", 81, "FHD or UHD output", E("output", [(0, "fhd", True), (1, "uhd", True)], "Output resolution."),
       critical="the display restarts on its own when the value changes")
simple("live_mode", 0x21, 0x09, "2.1.21.09", 82, "live mode", E("mode", [(0, "normal", True), (1, "live", True)], "Mode."),
       critical="the display restarts on its own when the value changes")
simple("dynamic_range_extension", 0x21, 0x0A, "2.1.21.0A", 83, "HDR dynamic range extension",
       E("level", [(0, "off", True), (1, "low", True), (2, "medium", True), (3, "high", True)], "Extension level."),
       critical="the display restarts on its own when the value changes")
D("screen_position", 0x21, 0x0B, "2.1.21.0B", 84, "the screen position",
  set=[U16("x", 0, 65535, "Horizontal position in pixels."), U16("y", 0, 65535, "Vertical position in pixels.")])
D("multilink_hdr", 0x21, 0x0C, "2.1.21.0C", 85, "MultiLink HDR",
  set=[ON("enabled", "On joins the display to a MultiLink HDR group."),
       U8("devices", 0, 255, "Number of displays in the group."), U8("device_id", 0, 255, "This display's ID in the group.")])
simple("color_enhancement", 0x21, 0x50, "2.1.21.50", 86, "color enhancement", ON("enabled", "On turns color enhancement on."))
simple("dynamic_backlight", 0x21, 0x51, "2.1.21.51", 87, "dynamic backlight",
       E("level", [(0, "off", True), (1, "low", True), (2, "standard", True), (3, "high", True)], "Level."))
simple("fit_to_screen", 0x21, 0x52, "2.1.21.52", 88, "fit to screen", E("mode", [(0, "off", True), (1, "on", True), (2, "auto", True)], "Mode."))
simple("uniformity", 0x21, 0x53, "2.1.21.53", 89, "uniformity correction", ON("enabled", "On turns uniformity correction on."),
       critical="the display restarts on its own when the value changes")
simple("gamma_mode", 0x21, 0x54, "2.1.21.54", 90, "the HDR gamma mode",
       E("mode", [(0, "hlg", True), (1, "st2084", True), (2, "bt1886", True), (3, "s_curve", True)], "Gamma curve."))
simple("black_equalizer", 0x21, 0x55, "2.1.21.55", 91, "the black equalizer",
       E("level", [(0, "off", True), (1, "low", True), (2, "high", True)], "Level."))
simple("hdr_plus", 0x21, 0x56, "2.1.21.56", 92, "HDR+", ON("enabled", "On turns HDR+ on."))
for key, cmd, page, what in [("contrast", 0x24, 93, "Contrast"), ("brightness", 0x25, 94, "Brightness"),
                             ("sharpness", 0x26, 95, "Sharpness"), ("color", 0x27, 96, "Color saturation")]:
    simple(key, cmd, None, f"2.1.{cmd:02X}", page, what.lower(), U8(key, 0, 100, f"{what}, 0-100."), poll="slow")
simple("tint", 0x28, None, "2.1.28", 97, "the tint", U8("tint", 0, 100, "Tint, 0-100 in steps of 2: the red share, green being 100 minus it."))
simple("coarse", 0x2F, None, "2.1.2F", 98, "PC coarse adjustment one step",
       E("direction", [(0, "down", True), (1, "up", True)], "Down or up one step."), get=False, set_name="step_coarse")
simple("fine", 0x30, None, "2.1.30", 99, "PC fine (phase) adjustment one step",
       E("direction", [(0, "down", True), (1, "up", True)], "Down or up one step."), get=False, set_name="step_fine")
simple("h_position", 0x31, None, "A.2", 313, "the PC horizontal position", U8("position", 0, 100, "0 leftmost, 50 centered, 100 rightmost."))
simple("v_position", 0x32, None, "A.3", 314, "the PC vertical position", U8("position", 0, 100, "0 top, 50 centered, 100 bottom."))
simple("auto_power", 0x33, None, "2.1.33", 102, "auto power", ON("enabled", "On powers the display on automatically."))
D("clear_menu", 0x34, None, "2.1.34", 103, "Close the on-screen menu", get=False, set=[LIT(0x00)], set_name="clear_menu")
simple("remote_control", 0x36, None, "2.1.36", 104, "the IR remote control", E("remote", [(0, "disabled", True), (1, "enabled", True)], "Whether the IR remote works."))
simple("rgb_contrast", 0x37, None, "2.1.37", 105, "the PC-source contrast", U8("contrast", 0, 100, "Contrast, 0-100."))
simple("rgb_brightness", 0x38, None, "2.1.38", 106, "the PC-source brightness", U8("brightness", 0, 100, "Brightness, 0-100."))
simple("pip", 0x3C, None, "2.1.3C", 107, "picture-in-picture", ON("enabled", "On shows the PIP window.", label="PIP"))
D("auto_adjustment", 0x3D, None, "2.1.3D", 108, "Run the PC auto adjustment once", get=False, set=[LIT(0x00)], set_name="auto_adjust")
simple("color_tone", 0x3E, None, "2.1.3E", 109, "the color tone", E("tone", COLOR_TONE, "Color tone; off lets set_color_temperature apply."))
simple("color_temperature", 0x3F, None, "2.1.3F", 110, "the color temperature",
       E("temperature", COLOR_TEMP, "Color temperature such as 6500k, or a legacy_ code of the older table; applies while the color tone is off."))
simple("pip_source", 0x40, None, "2.1.40", 112, "the PIP source", E("input", INPUTS, f"PIP source: {INPUTS_DESC}."))
simple("pip_size", 0x42, None, "2.1.42", 113, "the PIP size", E("size", PIP_SIZE, "PIP size."))
simple("pip_position", 0x43, None, "2.1.43", 114, "the PIP position",
       E("position", [(0, "off", False), (1, "upper_left", True), (2, "upper_right", True), (3, "lower_right", True), (4, "lower_left", True)], "Corner."))
simple("fan_speed", 0x44, None, "2.1.44", 115, "the fan speed (switching fan control to manual)", U8("speed", 0, 100, "Fan speed, 0-100."))
simple("auto_color", 0x45, None, "2.1.45", 116, "Run or reset the PC auto color",
       E("action", [(0, "reset", True), (1, "auto_color", True)], "auto_color runs it, reset restores the default."), get=False, set_name="auto_color")
simple("pip_sound", 0x47, None, "2.1.47", 117, "which picture's sound plays with PIP on", E("picture", [(0, "sub", True), (1, "main", True)], "Picture whose sound plays."))
simple("auto_volume", 0x48, None, "2.1.48", 118, "auto volume", E("mode", [(0, "off", True), (1, "normal", True), (2, "night", True)], "Mode."))
simple("standby", 0x4A, None, "2.1.4A", 119, "the standby setting", E("mode", [(0, "off", True), (1, "on", True), (2, "auto", True)], "Standby on no signal."))
D("picture_position", 0x4B, None, "2.1.4B", 120, "Move or resize the video picture one step, or reset it", get=False,
  set=[E("adjust", [(0, "reset", True), (1, "position", True), (2, "size", True)], "reset, position or size."),
       E("direction", [(0, "down", True), (1, "up", True), (2, "left", True), (3, "right", True)],
         "Direction: down/up move or scale vertically, left/right horizontally; ignored by reset.")],
  set_name="adjust_picture_position")
D("pixel_shift", 0x4C, None, "2.1.4C", 122, "pixel shift",
  set=[ON("enabled", "On shifts the picture to prevent burn-in."), U8("horizontal", 0, 4, "Horizontal dots, 0-4."),
       U8("vertical", 0, 4, "Vertical lines, 0-4."), U8("time", 1, 4, "Shift time code, 1-4.")])
D("light_sensor", 0x50, 0x00, "2.1.50.00", 124, "the light sensor", ack=[[U16("lux", 0, 65535, "Ambient light in lux.", unit="lx")]])
D("heatex_temperature", 0x50, 0x01, "2.1.50.01", 125, "the heat exchanger temperature", ack=[[I8("temperature", -60, 125, "Degrees Celsius.", unit="°C")]])
D("led_plate_temperature", 0x50, 0x02, "2.1.50.02", 126, "the LED plate temperature", ack=[[I8("temperature", -60, 125, "Degrees Celsius.", unit="°C")]])
D("final_duty", 0x50, 0x03, "2.1.50.03", 127, "the backlight final duty", ack=[[U16("duty", 0, 1023, "Final duty, 0-1023.")]])
for key, cmd, page, hz in [("eq_100hz", 0x51, 128, "100 Hz"), ("eq_300hz", 0x52, 129, "300 Hz"), ("eq_1khz", 0x53, 130, "1 kHz"),
                           ("eq_3khz", 0x54, 131, "3 kHz"), ("eq_10khz", 0x55, 132, "10 kHz")]:
    simple(key, cmd, None, f"2.1.{cmd:02X}", page, f"the {hz} equalizer band", U8("level", 0, 20, "Level, 0-20: 0 is -10, 10 is 0, 20 is +10."))
simple("energy_saving_lfd", 0x56, None, "2.1.56", 133, "energy saving (max power saving)", ON("enabled", "On turns energy saving on."))
D("auto_lamp", 0x57, None, "2.1.57", 134, "the automatic lamp (backlight) schedule", set=list(_lamp[:8]))
simple("manual_lamp", 0x58, None, "2.1.58", 135, "the manual lamp (backlight) value", U8("lamp", 0, 100, "Backlight, 0-100."))
simple("safety_screen_run", 0x59, None, "2.1.59", 136, "Run a safety screen now",
       E("type", [(0, "off", True), (1, "signal_pattern", True), (2, "all_white", True), (3, "scroll", True), (4, "bar", True),
                  (6, "eraser", True), (7, "pixel", True), (0x10, "rolling_bar", True), (0x11, "fading_screen", True)], "Pattern; off stops it."))
simple("inverse", 0x5A, None, "2.1.5A", 137, "inverse", ON("enabled", "On inverts the picture (a panel control on some models)."))
D("safety_screen", 0x5B, None, "2.1.5B", 138, "the safety screen timer",
  set=[E("type", [t for t in SAFETY_SCREEN_TYPES if t[0] < 0x80], "Pattern; off stops the timer."),
       U8("period", 0, 10, "Repeat period in hours, 1-10; 0 is off.", unit="h"),
       E("time", [(1, "10s", True), (2, "20s", True), (3, "30s", True), (4, "40s", True), (5, "50s", True)], "Run time.")],
  ack=[[E("type", SAFETY_SCREEN_TYPES, "Pattern."), U8("period", 0, 10, "Repeat period in hours.", unit="h"),
        E("time", [(1, "10s", True), (2, "20s", True), (3, "30s", True), (4, "40s", True), (5, "50s", True)], "Run time.")],
       [E("type", SAFETY_SCREEN_TYPES, "Pattern."), *HOUR12("start_", "start"), *HOUR12("end_", "end")]],
  set_summary="Set the safety screen timer to repeat: a pattern every period hours")
D("safety_screen_interval", 0x5B, None, "2.1.5B", 138, "the safety screen timer between two times", get=False,
  set=[E("type", [t for t in SAFETY_SCREEN_TYPES if t[0] >= 0x80], "Pattern, the interval kinds."),
       *HOUR12("start_", "start"), *HOUR12("end_", "end")],
  ack=DEFS[-1].ack, state="safety_screen",
  set_summary="Set the safety screen timer to run a pattern between a start and an end time")
simple("video_wall_mode", 0x5C, None, "2.1.5C", 140, "the video wall mode", E("mode", [(0, "natural", True), (1, "full", True)], "Mode."))
simple("safety_lock", 0x5D, None, "2.1.5D", 141, "safety lock", ON("locked", "On locks the display."))
simple("panel_lock", 0x5F, None, "2.1.5F", 142, "the panel button lock", ON("locked", "On locks the display's buttons.", off="unlocked", on="locked"))
simple("channel_step", 0x61, None, "2.1.61", 143, "Change the TV channel one step", E("direction", [(0, "up", True), (1, "down", True)], "Up or down."),
       get=False, set_name="step_channel")
simple("volume_step", 0x62, None, "2.1.62", 144, "Change the volume one step", E("direction", [(0, "up", True), (1, "down", True)], "Up or down."),
       get=False, set_name="step_volume")
D("ticker", 0x63, None, "2.1.63", 145, "the ticker (scrolling message)",
  set=[ON("enabled", "On shows the ticker."), *HOUR12("start_", "ticker start"), *HOUR12("end_", "ticker end"),
       E("horizontal", [(0, "center", True), (1, "left", True), (2, "right", True)], "Horizontal position."),
       E("vertical", [(0, "middle", True), (1, "top", True), (2, "bottom", True)], "Vertical position."),
       ON("motion", "On moves the text."),
       E("direction", [(0, "left", True), (1, "right", True), (2, "up", True), (3, "down", True)], "Motion direction."),
       E("speed", [(0, "normal", True), (1, "slow", True), (2, "fast", True)], "Motion speed."),
       E("font_size", [(0, "standard", True), (1, "small", True), (2, "large", True)], "Font size."),
       E("foreground", COLORS := [(0, "black", True), (1, "white", True), (2, "red", True), (3, "green", True), (4, "blue", True),
                                  (5, "yellow", True), (6, "magenta", True), (7, "cyan", True)], "Text color."),
       E("background", COLORS, "Background color."),
       E("foreground_opacity", [(0, "solid", True), (1, "transparent", True), (2, "translucent", True), (3, "flashing", True),
                                (4, "flash_all", True), (5, "off", True)], "Text opacity or flashing."),
       E("background_opacity", [(0, "solid", True), (1, "transparent", True), (2, "translucent", True)], "Background opacity."),
       TEXT("message", 232, "Message text, up to 232 bytes.")])
simple("sound_select", 0x65, None, "2.1.65", 148, "which picture's sound plays with PIP on (second command)",
       E("picture", [(0, "sub", True), (1, "main", True)], "Picture whose sound plays."))
D("pc_module", 0x66, None, "2.1.66", 149, "which PC module is detected", poll="identity",
  ack=[[E("module", [(0, "none", True), (1, "magicinfo", True), (2, "plug_in_module", True)], "Detected module.")]])
simple("device_name", 0x67, None, "2.1.67", 150, "the device name", TEXT("name", 15, "Device name, up to 15 characters."))
simple("speaker", 0x68, None, "2.1.68", 151, "the speaker", E("speaker", [(0, "internal", True), (1, "external", True)], "Speaker used."))
simple("osd", 0x70, None, "2.1.70", 152, "on-screen messages", ON("enabled", "On shows on-screen messages.", label="OSD"))
simple("picture_mode", 0x71, None, "2.1.71", 153, "the picture mode", E("mode", PMODE, "Picture mode; which apply depends on the model and source."))
simple("sound_mode", 0x72, None, "2.1.72", 155, "the sound mode",
       E("mode", [(0, "standard", True), (1, "music", True), (2, "movie", True), (3, "speech", True), (4, "custom", True),
                  (5, "amplify", True), (6, "optimized", True)], "Sound mode."))
simple("noise_reduction", 0x73, None, "2.1.73", 156, "digital noise reduction",
       E("level", [(0, "off", True), (1, "low", True), (2, "medium", True), (3, "high", True), (4, "auto", True),
                   (5, "auto_visualization", True)], "Level."))
simple("pc_color_tone", 0x75, None, "2.1.75", 157, "the PC-source color tone", E("tone", PC_COLOR_TONE, "Color tone."))
simple("auto_auto_adjustment", 0x76, None, "2.1.76", 158, "automatic PC auto adjustment", ON("enabled", "On lets the display auto adjust PC sources."))
simple("all_keys_lock", 0x77, None, "2.1.77", 159, "the lock of all keys (remote and buttons)", ON("locked", "On locks every key."))
simple("srs", 0x78, None, "2.1.78", 160, "SRS TruSurround XT", ON("enabled", "On turns SRS TruSurround XT on."))
simple("film_mode", 0x79, None, "2.1.79", 161, "film mode",
       E("mode", [(0, "off", True), (1, "auto1", True), (2, "auto2", True), (3, "cinema_smooth", True)], "Mode."))
D("panel_on_time", 0x83, None, "2.1.83", 162, "the panel's operating time", ack=[[U16("count", 0, 65535, "Operating time counter; it rises every 10 minutes.")]])
simple("video_wall", 0x84, None, "2.1.84", 163, "the video wall", ON("enabled", "On turns the video wall on.", label="Video wall"))
simple("temperature_limit", 0x85, None, "2.1.85", 164, "the temperature above which the display powers off",
       U8("limit", 75, 124, "Temperature limit, 75-124 (degrees Celsius).", unit="°C"))
simple("brightness_sensor", 0x86, None, "2.1.86", 165, "the brightness (eco) sensor", ON("enabled", "On lets the sensor adjust brightness."))
simple("dynamic_contrast", 0x87, None, "2.1.87", 166, "dynamic contrast",
       E("level", [(0, "off", True), (1, "low", True), (2, "medium", True), (3, "high", True)], "Level."))
D("video_wall_layout", 0x89, None, "2.1.89", 167, "the video wall division and this display's place in it",
  set=[E("division", WALL_DIV, "Wall division as HxV, such as 2x2 (columns by rows, up to 15x15), or off."),
       U8("position", 1, 225, "This display's position in the wall, numbered from 1.")])
D("model_name", 0x8A, None, "2.1.8A", 168, "the model name", poll="identity", ack=[[TEXT("name", 255, "Model name.")]])
D("video_wall_settings", 0x8B, None, "2.1.8B", 169, "the video wall, mode, layout and input in one command",
  set=[ON("enabled", "On turns the video wall on."), E("mode", [(0, "natural", True), (1, "full", True)], "Mode."),
       E("division", WALL_DIV, "Wall division as HxV, or off."), U8("position", 1, 225, "This display's position, from 1."),
       E("input", INPUTS, f"Input shown: {INPUTS_DESC}.")])
simple("frame_alignment", 0x8C, 0x81, "2.1.8C.81", 171, "video wall frame alignment",
       E("mode", [(0, "off", True), (1, "on", True), (2, "auto", True)], "Mode."))
simple("fan", 0x8F, None, "2.1.8F", 172, "fan control", E("mode", [(0, "manual", True), (1, "auto", True), (2, "off", True), (3, "on", True)], "Mode."))
simple("game_mode", 0x90, None, "2.1.90", 173, "game mode", ON("enabled", "On turns game mode on."))
simple("energy_saving", 0x92, None, "2.1.92", 174, "energy saving",
       E("level", [(0, "off", True), (1, "low", True), (2, "medium", True), (3, "high", True), (4, "picture_off", True)], "Level; picture_off turns the screen off."))
simple("hdmi_black_level", 0x94, None, "2.1.94", 175, "the HDMI black level", E("level", [(0, "normal", True), (1, "low", True), (2, "auto", True)], "Level."))
simple("black_adjust", 0x95, None, "2.1.95", 176, "black adjust (black tone)",
       E("level", [(0, "off", True), (1, "low", True), (2, "medium", True), (3, "high", True)], "Level (dark, darker, darkest on black tone models)."))
simple("gamma", 0x96, None, "2.1.96", 177, "gamma", E("gamma", GAMMA, "Gamma: natural, mode1-mode5, minus1-minus5 or custom."))
simple("edge_enhancement", 0x9C, None, "2.1.9C", 178, "edge enhancement", ON("enabled", "On turns edge enhancement on."))
simple("color_space", 0x9D, None, "2.1.9D", 179, "the color space",
       E("space", [(0, "auto", True), (1, "native", True), (2, "custom", True), (3, "dci_p3", True), (4, "adobe_rgb", True),
                   (5, "bt709", True)], "Color space."))
simple("xvycc", 0x9E, None, "2.1.9E", 180, "xvYCC", ON("enabled", "On turns xvYCC on."))
simple("reset", 0x9F, None, "2.1.9F", 181, "Reset settings to their defaults",
       E("what", [(0, "picture", True), (1, "sound", True), (2, "setup", True), (3, "all", True), (4, "screen_display", True)],
         "Which settings to reset."), get=False, set_name="reset_settings", critical="settings reset to defaults cannot be restored")
D("ambient_brightness", 0xA1, None, "2.1.A1", 182, "ambient brightness mode",
  set=[ON("enabled", "On turns ambient brightness mode on."), ON("apply_lamp", "On applies the lamp value too."),
       U8("lamp", 0, 100, "Lamp value, 0-100.")],
  ack=[[ON("enabled", "Ambient brightness mode on."), U8("lamp", 0, 100, "Lamp value, 0-100.")]])
D("osd_types", 0xA3, None, "2.1.A3", 183, "each kind of on-screen message",
  set=[E("type", [(0, "source", True), (1, "not_optimum_mode", True), (2, "no_signal", True), (3, "mdc", True),
                  (4, "schedule_channel", True)], "Which on-screen message."), ON("enabled", "On shows it.")],
  ack=[[FLAGS("shown", [(0, "source"), (1, "not_optimum_mode"), (2, "no_signal"), (3, "mdc"), (4, "schedule_channel")],
              "Which kinds of on-screen message are shown.")]],
)
TIMER_FIELDS = [*HOUR12("on_", "on time"), ON("on_enabled", "On activates the on time."), *HOUR12("off_", "off time"),
                ON("off_enabled", "On activates the off time."), E("repeat", TIMER_REPEAT, "Days the timer runs; manual uses days."),
                U8("days", 0, 127, "With repeat manual, the days as a bit mask: 1 Sunday, 2 Monday, 4 Tuesday, 8 Wednesday, 16 Thursday, 32 Friday, 64 Saturday."),
                U8("volume", 0, 100, "Volume at the on time, 0-100."), E("input", INPUTS, f"Input at the on time: {INPUTS_DESC}."),
                E("holidays", HOLIDAY_APPLY, "Whether holidays skip it: none, both, on_timer_only or off_timer_only.")]
TIMER_SEPARATE = [*HOUR12("on_", "on time"), ON("on_enabled", "On activates the on time."),
                  E("on_repeat", TIMER_REPEAT, "Days the on time runs."), U8("on_days", 0, 127, "On time days bit mask (1 Sunday ... 64 Saturday)."),
                  *HOUR12("off_", "off time"), ON("off_enabled", "On activates the off time."),
                  E("off_repeat", TIMER_REPEAT, "Days the off time runs."), U8("off_days", 0, 127, "Off time days bit mask."),
                  U8("volume", 0, 100, "Volume at the on time, 0-100."), E("input", INPUTS, "Input at the on time."),
                  E("holidays", HOLIDAY_APPLY, "Whether holidays skip it.")]
for n, cmd, page in [(1, 0xA4, 184), (2, 0xA5, 186), (3, 0xA6, 188), (4, 0xAB, 193), (5, 0xAC, 195), (6, 0xAD, 197), (7, 0xAE, 199)]:
    D(f"timer{n}", cmd, None, f"2.1.{cmd:02X}", page, f"on/off timer {n}", set=TIMER_FIELDS, ack=[TIMER_FIELDS, TIMER_SEPARATE])
    D(f"timer{n}_separate", cmd, None, f"2.1.{cmd:02X}", page, f"on/off timer {n} with separate on and off days", get=False,
      set=TIMER_SEPARATE, ack=[TIMER_FIELDS, TIMER_SEPARATE], state=f"timer{n}",
      set_summary=f"Set on/off timer {n} on a display whose timers have separate on and off repeat days (§2.1.{cmd:02X}, LEN 0x0F)")
D("clock", 0xA7, None, "2.1.A7", 190, "the clock",
  set=[U8("day", 1, 31, "Day of the month."), U8("hour", 1, 12, "Hour, 1-12."), U8("minute", 0, 59, "Minute."),
       U8("month", 1, 12, "Month, 1-12."), U16("year", 0, 65535, "Year, such as 2026."), E("ampm", AMPM, "AM or PM.")])
D("holiday", 0xA8, None, "2.1.A8", 191, "a holiday", get=False,
  set=[LIT(0x00), U8("start_month", 1, 12, "First month."), U8("start_day", 1, 31, "First day."),
       U8("end_month", 1, 12, "Last month."), U8("end_day", 1, 31, "Last day.")], set_name="add_holiday",
  ack=[[F("data", "hex", "The display's answer, as hex.")]], set_summary="Add a holiday period, when timers set to skip holidays do not run")
D("holiday_delete", 0xA8, None, "2.1.A8", 191, "a holiday", get=False,
  set=[LIT(0x01), U8("start_month", 1, 12, "First month."), U8("start_day", 1, 31, "First day."),
       U8("end_month", 1, 12, "Last month."), U8("end_day", 1, 31, "Last day.")], set_name="delete_holiday",
  ack=[[F("data", "hex", "The display's answer, as hex.")]], set_summary="Delete a holiday period", critical="the holiday is removed")
D("holiday_clear", 0xA8, None, "2.1.A8", 191, "all holidays", get=False, set=[LIT(0x02), LIT(0), LIT(0), LIT(0), LIT(0)],
  set_name="delete_all_holidays", ack=[[F("data", "hex", "The display's answer, as hex.")]],
  set_summary="Delete every holiday", critical="every holiday is removed")
D("holiday_count", 0xA9, None, "2.1.A9", 192, "the number of holidays",
  ack=[[U8("count", 0, 255, "Number of holidays."), SKIP(4)]])
D("holidays", 0xA9, None, "2.1.A9", 192, "one holiday by index", poll="never",
  get_args=[U8("index", 0, 255, "Holiday number, as the display counts them.")],
  ack=[[U8("index", 0, 255, "Holiday number."), U8("start_month", 1, 12, "First month."), U8("start_day", 1, 31, "First day."),
        U8("end_month", 1, 12, "Last month."), U8("end_day", 1, 31, "Last day.")]],
  get_summary="Read one holiday by its number (Holiday Get Control with an index, §2.1.A9); the reply goes to holidays.<index>",
  state="holidays.{index}")
simple("input_name", 0xAF, None, "2.1.AF", 202, "the edit name of the current input",
       E("name", [(0, "none", True), (1, "vcr", True), (2, "dvd", True), (3, "cable_stb", True), (4, "satellite_stb", True),
                  (5, "pvr_stb", True), (6, "av_receiver", True), (7, "game", True), (8, "camcorder", True), (9, "pc", True),
                  (0x0A, "dvi_pc", True), (0x0B, "dvi_devices", True), (0x0C, "tv", True), (0x0D, "iptv", True),
                  (0x0E, "blu_ray", True), (0x0F, "hd_dvd", True), (0x10, "dma", True), (0x11, "dvd_receiver", True),
                  (0x12, "hd_stb", True), (0x13, "dvd_combo", True), (0x14, "dhr", True)], "Edit name."))
simple("remote_key", 0xB0, None, "2.1.B0", 203, "Press a remote control key", E("key", KEYS, "The key."), get=False, set_name="press_key")
simple("daisy_chain", 0xB1, None, "2.1.B1", 205, "DisplayPort daisy chain", E("mode", [(0, "clone", True), (1, "expand", True)], "Mode."))
_split3 = [E("mode", SPLIT_ON, "Off, 3_screen or 4_screen."), E("sound", SPLIT_SOUND, "Screen whose sound plays."),
           E("layout", SPLIT_SIZE, "Screen size mode."), E("main_picture", SPLIT_PICTURE, "Main screen picture size."),
           E("sub1_input", INPUTS, "Sub screen 1 input."), E("sub1_picture", SPLIT_PICTURE, "Sub screen 1 picture size."),
           E("sub2_input", INPUTS, "Sub screen 2 input."), E("sub2_picture", SPLIT_PICTURE, "Sub screen 2 picture size.")]
_split4 = _split3 + [E("sub3_input", INPUTS, "Sub screen 3 input."), E("sub3_picture", SPLIT_PICTURE, "Sub screen 3 picture size.")]
_split4b = [E("mode", SPLIT_ON, "Off, 3_screen or 4_screen."), E("sound", SPLIT_SOUND, "Screen whose sound plays."),
            E("sub0_input", INPUTS, "Top-left screen input."), E("sub1_input", INPUTS, "Top-right screen input."),
            E("sub2_input", INPUTS, "Bottom-left screen input."), E("sub3_input", INPUTS, "Bottom-right screen input.")]
D("multi_screen", 0xB2, None, "2.1.B2", 206, "3-screen mode (type 1)", set=_split3, ack=[_split3, _split4, _split4b])
D("multi_screen_4", 0xB2, None, "2.1.B2", 206, "4-screen mode (type 2)", get=False, set=_split4, ack=[_split3, _split4, _split4b],
  state="multi_screen")
D("multi_screen_quad", 0xB2, None, "2.1.B2", 206, "4-screen mode without picture sizes (type 3)", get=False, set=_split4b,
  ack=[_split3, _split4, _split4b], state="multi_screen")
simple("conference_sound", 0xB3, None, "2.1.B3", 209, "video conference sound mode", ON("enabled", "On turns video conference sound on."))
simple("network_standby", 0xB5, None, "2.1.B5", 210, "network standby", ON("enabled", "On keeps the network up in standby."))
D("dst", 0xB6, None, "2.1.B6", 211, "daylight saving time",
  set=[E("mode", [(0, "off", True), (1, "auto", True), (2, "on", True)], "off, auto (tuner models) or on (manual)."),
       U8("start_month", 0, 11, "Start month, 0 January to 11 December."),
       E("start_week", [(0, "1st", True), (1, "2nd", True), (2, "3rd", True), (3, "4th", True), (4, "last", True)], "Week of the start month."),
       U8("start_weekday", 0, 6, "Start weekday, 0 Monday to 6 Sunday."), U8("start_hour", 0, 23, "Start hour, 0-23."),
       U8("start_minute", 0, 59, "Start minute."), U8("end_month", 0, 11, "End month, 0 January to 11 December."),
       E("end_week", [(0, "1st", True), (1, "2nd", True), (2, "3rd", True), (3, "4th", True), (4, "last", True)], "Week of the end month."),
       U8("end_weekday", 0, 6, "End weekday, 0 Monday to 6 Sunday."), U8("end_hour", 0, 23, "End hour, 0-23."),
       U8("end_minute", 0, 59, "End minute."), E("offset", [(0, "1h", True), (1, "2h", True)], "Time offset.")],
  ack=None)
DEFS[-1].ack = [DEFS[-1].set + [E("tuner", [(0, "tuner", True), (1, "tunerless", True)], "Whether the display has a tuner.")], DEFS[-1].set]
D("custom_pip", 0xB7, None, "2.1.B7", 213, "the custom PIP window",
  set=[U16("x", 0, 65535, "Horizontal start in pixels (10-pixel steps)."), U16("y", 0, 65535, "Vertical start in pixels (10-pixel steps)."),
       U16("width", 0, 65535, "Width in pixels."), U16("height", 0, 65535, "Height in pixels.")])
simple("auto_id_status", 0xB8, None, "2.1.B8", 214, "the auto ID setting status", E("status", [(0, "start", True), (1, "end", True)], "Start or end."))
simple("show_id", 0xB9, None, "2.1.B9", 215, "the display ID on screen", ON("shown", "On shows this display's ID on screen."), get=False, set_name="show_id")
D("clock_seconds", 0xC5, None, "2.1.C5", 216, "the clock, with seconds",
  set=[U8("day", 1, 31, "Day of the month."), U8("hour", 1, 12, "Hour, 1-12."), U8("minute", 0, 59, "Minute."), U8("second", 0, 59, "Second."),
       U8("month", 1, 12, "Month, 1-12."), U16("year", 0, 65535, "Year, such as 2026."), E("ampm", AMPM, "AM or PM.")])
simple("auto_power_off", 0xC6, 0x81, "2.1.C6.81", 218, "eco auto power off",
       E("after", [(0, "off", True), (1, "4h", True), (2, "6h", True), (3, "8h", True), (4, "16h", True)], "Hours before power off, or off (4h reads as on where only on/off exists)."))
simple("brightness_limit", 0xC6, 0x82, "2.1.C6.82", 219, "the eco brightness limit", ON("enabled", "On limits brightness."))
simple("play_via", 0xC7, 0x81, "2.1.C7.81", 221, "what the launcher plays", E("mode", [(0, "magicinfo", True), (1, "url_launcher", True), (2, "magic_iwb", True)], "Player."))
simple("launcher_url", 0xC7, 0x82, "2.1.C7.82", 222, "the URL launcher address", TEXT("url", 200, "URL, up to 200 ASCII characters."))
simple("menu_orientation", 0xC8, 0x81, "2.1.C8.81", 224, "the menu orientation", E("orientation", ORIENTATION, "Orientation."))
simple("source_orientation", 0xC8, 0x82, "2.1.C8.82", 225, "the source content orientation", E("orientation", ORIENTATION, "Orientation."))
simple("rotated_aspect", 0xC8, 0x83, "2.1.C8.83", 226, "the aspect ratio when rotated", E("aspect", [(0, "full_screen", True), (1, "original", True)], "Aspect ratio."))
simple("pip_rotation", 0xC8, 0x84, "2.1.C8.84", 227, "the PIP orientation", E("orientation", ORIENTATION, "Orientation."))
simple("menu_size", 0xC8, 0x85, "2.1.C8.85", 228, "the menu size", E("size", [(0, "original", True), (1, "medium", True), (2, "small", True)], "Size."))
simple("hdmi_sound", 0xC9, 0x81, "2.1.C9.81", 230, "the HDMI sound source", E("source", [(0, "hdmi", True), (1, "audio_in", True)], "hdmi plays the HDMI signal's sound, audio_in the audio input."))
for key, sub, page, hz in [("eq_200hz", 0x82, 231, "200 Hz"), ("eq_500hz", 0x83, 232, "500 Hz"), ("eq_2khz", 0x84, 233, "2 kHz"), ("eq_5khz", 0x85, 234, "5 kHz")]:
    simple(key, 0xC9, sub, f"2.1.C9.{sub:02X}", page, f"the {hz} equalizer band", U8("level", 0, 20, "Level, 0-20: 0 is -10, 10 is 0, 20 is +10."))
D("sbox_mode", 0xCA, 0x60, "2.1.CA.60", 236, "the signal box mode", led=True, poll="identity",
  ack=[[E("mode", [(0, "indoor", True), (1, "outdoor", True)], "Mode.")]])
simple("dimming_mode", 0xCA, 0x61, "2.1.CA.61", 237, "the LED dimming mode",
       E("mode", [(0, "auto", True), (1, "light_sensor", True), (2, "sunrise_sunset", True), (3, "off", True)], "Mode."), led=True)
simple("night_constant_brightness", 0xCA, 0x62, "2.1.CA.62", 238, "night time constant brightness", ON("enabled", "On keeps brightness constant at night."), led=True)
simple("brightness_change_period", 0xCA, 0x63, "2.1.CA.63", 239, "the brightness change period", U8("minutes", 10, 70, "Period in minutes, 10-70.", unit="min"), led=True)
D("light_sensor_range", 0xCA, 0x64, "2.1.CA.64", 240, "the light sensor's effective range", led=True,
  set=[LIT(0x00), U16("minimum", 0, 65535, "Lowest effective light in lux.", unit="lx"),
       LIT(0x01), U16("maximum", 0, 65535, "Highest effective light in lux.", unit="lx")])
D("brightness_output_range", 0xCA, 0x65, "2.1.CA.65", 241, "the brightness output range and default", led=True,
  set=[LIT(0x00), U16("minimum", 0, 100, "Lowest output, percent.", unit="%"), LIT(0x01), U16("maximum", 0, 100, "Highest output, percent.", unit="%"),
       LIT(0x02), U16("default", 0, 100, "Default output, percent.", unit="%")])
D("location", 0xCA, 0x66, "2.1.CA.66", 242, "the latitude and longitude", led=True,
  set=[LIT(0x00), F("latitude", "ptext", "Latitude as text.", n=32), LIT(0x01), F("longitude", "ptext", "Longitude as text.", n=32)])
simple("cec", 0xCA, 0x70, "2.1.CA.70", 243, "HDMI-CEC", ON("enabled", "On turns HDMI-CEC on.", label="CEC"))
D("device_group", 0xCA, 0x71, "2.1.CA.71", 244, "multi-device grouping",
  set=[U8("group", 0, 255, "Group number; 0 is off."), E("role", [(0, "sub", True), (1, "main", True)], "Role in the group.")])
simple("auto_source_switch", 0xCA, 0x81, "2.1.CA.81", 245, "auto source switching", ON("enabled", "On switches to the preset input automatically."))
D("auto_source", 0xCA, 0x82, "2.1.CA.82", 246, "the auto source switching inputs",
  set=[ON("primary_recovery", "On returns to the primary input when it comes back."),
       E("primary", [(0, "all", True)] + INPUTS, "Primary input, or all."), E("secondary", INPUTS, "Secondary input.")])
simple("power_on_delay", 0xCA, 0x83, "2.1.CA.83", 247, "the power on delay", U8("seconds", 0, 255, "Delay in seconds.", unit="s"))
simple("synced_power_on", 0xCA, 0x84, "2.1.CA.84", 248, "synced power on", ON("enabled", "On powers on with the chain."))
simple("synced_power_off", 0xCA, 0x85, "2.1.CA.85", 249, "synced power off", ON("enabled", "On powers off with the chain."))
simple("power_button", 0xCA, 0x91, "2.1.CA.91", 250, "what the power button does", E("mode", [(0, "on_only", True), (1, "on_off", True)], "Power on only, or on and off."))
simple("touch_lock", 0xCA, 0x92, "2.1.CA.92", 251, "the touch control admin lock", ON("locked", "On locks touch control."))
simple("dicom", 0xCA, 0x93, "2.1.CA.93", 252, "DICOM mode", ON("enabled", "On turns DICOM mode on.", label="DICOM"))
simple("no_signal_power_off", 0xCA, 0xA1, "2.1.CA.A1", 253, "power off after no signal",
       E("after", [(0, "off", True), (1, "15min", True), (2, "30min", True), (3, "60min", True), (4, "10min", True)], "Delay, or off."))
simple("minimal_backlight", 0xCA, 0xB0, "2.1.CA.B0", 254, "the eco sensor's minimal backlight", U8("level", 0, 255, "Minimal backlight level."))
D("led_info", 0xD0, 0x78, "2.1.D0.78", 257, "the LED wall's picture settings and versions", led=True,
  ack=[[LIT(0x01), E("gamut", GAMUT, "Cabinet gamut."), U8("backlight", 0, 10, "Cabinet backlight, 0-10."),
        ON("cabinet_cc", "Cabinet color correction on."), ON("module_cc", "Module color correction on."),
        ON("seam_correction", "Seam correction on."), ON("dynamic_peaking", "Dynamic peaking on."),
        LIST("versions", [E("component", SW_DATA_TYPES, "Component."), F("version", "ptext", "Version.", n=255)],
             "Each component's version, {component, version}.", prefix=True)]])
D("led_device_type", 0xD0, 0x81, "2.1.D0.81", 259, "the LED device type", led=True, poll="identity",
  ack=[[E("type", [(1, "sendbox", True), (2, "cabinet_is", True), (3, "cabinet_ifj", True), (4, "cabinet_iwj", True),
                   (5, "cabinet_iwr", True), (6, "cabinet_ier", True), (7, "wall_2_0", True)], "Device type.")]])
_led_inputs = [(0, "hdbt"), (1, "dsub"), (2, "dvi"), (3, "hdmi"), (4, "dp"), (5, "media")]
D("led_inputs", 0xD0, 0x82, "2.1.D0.82", 260, "the LED box's inputs and signal", led=True,
  ack=[[FLAGS("supported", _led_inputs, "Inputs the box has."), FLAGS("connected", _led_inputs, "Inputs connected (media: player running)."),
        E("current", INPUTS, "Current input."), U16("width", 0, 65535, "Current signal width in pixels."),
        U16("height", 0, 65535, "Current signal height in pixels.")]])
_product = [E("pitch", PITCH, "Pixel pitch in millimeters."), U8("resolution", 0, 255, "Cabinet resolution code (§2.1.D0.83)."),
            E("size", LED_PHY_SIZE, "Cabinet size in millimeters."), E("aspect", LED_ASPECT, "Cabinet aspect ratio."),
            E("modules", LED_MODULES, "Module arrangement.")]
D("led_product", 0xD0, 0x83, "2.1.D0.83", 261, "the LED cabinet's product information", led=True, poll="identity", ack=[_product])
_power_ic = [(0, "1v2"), (1, "1v8"), (2, "3v3"), (3, "5v0"), (4, "4v2"), (5, "power_detect_ic"), (6, "stm32"), (7, "fpga")]
_hdbt = [(0, "input_connected"), (1, "input_signal"), (2, "rx_ok"), (4, "output_connected"), (5, "output_signal"), (6, "tx_ok")]
_monitor = [FLAGS("power_ic", _power_ic, "Power rail and IC status bits."), FLAGS("hdbt", _hdbt, "HDBaseT status."),
            U8("temperature", 0, 254, "Temperature (degrees Celsius)."), U8("illuminance", 0, 100, "Illuminance, 0-100; absent without a sensor."),
            LIST("module_errors", [U8("module", 0, 255, "Module code (0x1E module 1 ... 0xCE module 12)."), U8("errors", 0, 255, "LED error count.")],
                 "Each module's LED error count, {module, errors}.")]
D("led_monitoring", 0xD0, 0x84, "2.1.D0.84", 263, "the LED cabinet's health", led=True, ack=[_monitor])
simple("abl", 0xD0, 0x85, "2.1.D0.85", 265, "ABL mode", ON("enabled", "On turns ABL mode on.", label="ABL"), led=True)
simple("scanning_rate", 0xD0, 0x86, "2.1.D0.86", 266, "scanning rate mode", ON("enabled", "On turns scanning rate mode on."), led=True)
D("lod_recheck", 0xD0, 0x87, "2.1.D0.87", 267, "Recheck LED open detection", get=False, set=[LIT(0x00)], set_name="recheck_lod", led=True)
simple("cabinet_backlight", 0xD0, 0x94, "2.1.D0.94", 273, "the cabinet backlight", U8("level", 0, 10, "Backlight, 0-10."), led=True)
simple("pixel_cc", 0xD0, 0x95, "2.1.D0.95", 274, "cabinet pixel color correction", ON("enabled", "On turns pixel color correction on."), led=True)
simple("gamut", 0xD0, 0x96, "2.1.D0.96", 275, "the cabinet gamut", E("gamut", GAMUT, "Gamut: natural (off), srgb, adobe_rgb or dci_p3."), led=True)
simple("seam_correction", 0xD0, 0x98, "2.1.D0.98", 279, "cabinet seam correction", ON("enabled", "On turns seam correction on."), led=True)
simple("module_cc", 0xD0, 0x99, "2.1.D0.99", 280, "module color correction", ON("enabled", "On turns module color correction on."), led=True)
simple("data_reload", 0xD0, 0x9A, "2.1.D0.9A", 281, "Reload LED calibration data",
       E("data", [(1, "pixel_rgb", True), (2, "over_voltage_count", True), (3, "lod", True), (4, "rm_data", True)], "What to reload."),
       get=False, set_name="reload_led_data", led=True)
simple("block_cc", 0xD0, 0x9D, "2.1.D0.9D", 286, "block color correction", ON("enabled", "On turns block color correction on."), led=True)
simple("cabinet_cc", 0xD0, 0x9E, "2.1.D0.9E", 287, "cabinet color correction", ON("enabled", "On turns cabinet color correction on."), led=True)
simple("block_gradation", 0xD0, 0xA3, "2.1.D0.A3", 291, "block gradation", ON("enabled", "On turns block gradation on."), led=True)
D("led_diagnosis", 0xD0, 0xC2, "2.1.D0.C2", 292, "the LED diagnosis in one reply", led=True,
  ack=[[LIT(0x01), *_product, ON("abl", "ABL on."), ON("auto_source_switch", "Auto source switch on."), ON("osd", "On-screen messages on."), *_monitor]])
D("led_auto_id", 0xD0, 0xC3, "2.1.D0.C3", 294, "the cabinet auto ID", led=True,
  set=[LIT(0x01)], set_name="start_led_auto_id",
  ack=[[E("status", [(0, "in_progress", True), (1, "done", True)], "Progress."), U8("last_id", 0, 255, "ID given to the last cabinet found.")]],
  set_ack=[[LIT(0x01)]], critical="renumbers every cabinet")
D("net_pip", 0xE0, None, "2.1.E0", 298, "MagicInfo network PIP", get=False,
  set=[LIT(0x01), U16("x", 0, 65535, "Horizontal start in pixels."), U16("y", 0, 65535, "Vertical start in pixels."),
       U16("width", 0, 65535, "Width in pixels."), U16("height", 0, 65535, "Height in pixels."),
       E("input", INPUTS, f"PIP input: {INPUTS_DESC}."), U8("tv_channel", 0, 99, "TV channel, 0-99 (460TXn only)."),
       E("sound", [(0, "magicinfo", True), (1, "pip", True)], "Sound played."),
       E("country", [(0, "korea", True), (1, "usa", True)], "Channel table country."),
       E("band", [(0, "analog", True), (1, "digital", True)], "Analog or digital TV."),
       E("source", [(0, "air", True), (1, "cable", True)], "Air or cable."),
       U16("channel", 0, 65535, "Major channel (65535 the last channel)."),
       E("minor", [(0, "enabled", True), (1, "disabled", True)], "Whether the minor channel is used."),
       U16("minor_channel", 0, 65535, "Minor channel (65535 the last).")],
  ack=[[F("data", "hex", "The display's answer, as hex.")]], set_name="show_net_pip")
D("net_pip_off", 0xE0, None, "2.1.E0", 298, "MagicInfo network PIP", get=False, set=[LIT(0x00)], set_name="hide_net_pip",
  ack=[[F("data", "hex", "The display's answer, as hex.")]])
simple("apply_to", 0xE4, None, "2.1.E4", 300, "what the video wall applies to",
       E("target", [(0, "current_source", True), (1, "magicinfo_player_s", True)], "Current source or the MagicInfo Player S."))
simple("panel", 0xF9, None, "2.1.F9", 301, "the panel (screen) on or off", E("panel", [(0, "on", True), (1, "off", True)], "on lights the panel, off blanks it."))
D("auto_id", 0xFD, None, "2.1.FD", 302, "the RS-232C loop-out and the display ID",
  set=[U8("flags", 0, 255, "Bit 0 disables the RS-232C loop-out; bit 4 resets the display ID to 0."),
       U8("id", 0, 253, "New display ID, 1-253; 0 keeps the current one.")],
  critical="changing the display ID makes it stop answering the ID this device uses")
simple("white_balance_mode", 0xFE, 0x62, "2.1.FE.62", 304, "the white balance mode",
       E("mode", [(0, "custom", True), (1, "color_expert", True)], "Mode."))
for key, sub, page, what in [("red_gain", 0x81, 305, "Red gain"), ("green_gain", 0x91, 306, "Green gain"),
                             ("blue_gain", 0xA1, 307, "Blue gain"), ("red_offset", 0xB1, 308, "Red offset"),
                             ("green_offset", 0xC1, 309, "Green offset"), ("blue_offset", 0xD1, 310, "Blue offset")]:
    simple(key, 0xFE, sub, f"2.1.FE.{sub:02X}", page, f"the white balance {what.lower()}",
           U8("value", 0, 255, f"{what}; its range depends on the model."))
D("white_balance", 0xFE, None, "2.1.FE", 303, "the whole white balance of an input", get=False,
  set=[LIT(0x02), E("input", INPUTS, f"Input: {INPUTS_DESC}."), U8("red_gain", 0, 255, "Red gain."), U8("green_gain", 0, 255, "Green gain."),
       U8("blue_gain", 0, 255, "Blue gain."), U8("red_offset", 0, 255, "Red offset."), U8("green_offset", 0, 255, "Green offset."),
       U8("blue_offset", 0, 255, "Blue offset."), U8("sub_brightness", 0, 255, "Sub brightness."), U8("sub_contrast", 0, 255, "Sub contrast.")],
  ack=[[E("input", INPUTS, "Input."), U8("red_gain", 0, 255, "Red gain."), U8("green_gain", 0, 255, "Green gain."),
        U8("blue_gain", 0, 255, "Blue gain."), U8("red_offset", 0, 255, "Red offset."), U8("green_offset", 0, 255, "Green offset."),
        U8("blue_offset", 0, 255, "Blue offset."), U8("sub_brightness", 0, 255, "Sub brightness."), U8("sub_contrast", 0, 255, "Sub contrast.")]])
# LED white balance and seam commands whose bit layouts the document leaves
# illegible: the data is passed as hex, the reply stored as hex (see quirks).
for key, sub, page, what in [("module_wb", 0x92, 268, "module white balance (RGB) gains"),
                             ("cabinet_cc_gains", 0x93, 271, "cabinet color correction (RGB) gains"),
                             ("seam_offsets", 0x97, 276, "cabinet seam correction values"),
                             ("block_wb", 0x9B, 282, "block white balance (RGB) gains"),
                             ("cabinet_wb", 0x9C, 284, "cabinet white balance (RGB) gains"),
                             ("block_gradation_data", 0xA2, 289, "block gradation values")]:
    D(key, 0xD0, sub, f"2.1.D0.{sub:02X}", page, f"the {what}", led=True, poll="never",
      get_args=[F("query", "hex", "The bytes after the sub-command, as hex: the module, block or position to read (none for all).", n=64)],
      set=[F("data", "hex", "The bytes after the sub-command, as hex, laid out as the document's section gives them.", n=240)],
      ack=[[F("data", "hex", "The reply's bytes after the sub-command, as hex.")]])
D("edge_offset", 0xD0, 0x9F, "2.1.D0.9F", 288, "multiple edge offsets", led=True, get=False,
  set=[F("data", "hex", "The bytes after the sub-command, as hex: information, offset, then module and edge pairs.", n=240)],
  ack=[[F("data", "hex", "The reply's bytes after the sub-command, as hex.")]], set_name="set_edge_offset")

DEFS = [d for d in DEFS if d is not None]


# ── The spec ────────────────────────────────────────────────────────────────

HEADER = """\
# Generated by tools/generate_samsung_mdc.py: edit the generator, not this file.
spec: 1
id: samsung-mdc
name: Samsung displays (MDC, Multiple Display Control)
vendor: Samsung
category: display
implementation: native
reason: >
  MDC is binary: each request is 0xAA, a command byte, the display's ID, a data length, the data and a checksum
  (the sum of every byte after the header, modulo 256), and each answer is 0xAA 0xFF, the ID, a length, 'A' or
  'N', the command it answers and its values, so replies are framed by their length and matched to the request
  by command and ID, one request at a time. Many replies have several layouts told apart only by their length
  (timers, maintenance, sound, multi-screen). Displays on an RS-232C chain share one port and are addressed by
  ID. The secured mode of newer firmware greets with plain text, then takes a TLS handshake and a PIN on the
  same connection. Nothing is pushed, so state comes from a paced poll.

source:
  - title: "Multiple Display Control Protocol, Ver. 15.0 (2020-11-06), SEC-VD-DSW"
    author: "Samsung Electronics, Display S/W, Video Display Division"
    url: "https://electis.co.il/files/LFD%20Applications/MDC/MDC_Protocol_2020_mdc_ppmxxm6x_Protocolv15.0c.pdf"
    note: >
      Samsung's customer document for its signage displays (316 pages), read from a Samsung display
      distributor's copy, since Samsung serves it to partners rather than from a public page. Nothing here is
      quoted from it; wire layouts are restated. Connection: RS-232C 9600 8N1 on pins 2, 3 and 5, IDs 0-253
      unique on a chain, 0xFE addressing every display with no ACK (§1.1, §1.2.1); RJ45: the same frames over
      TCP 1515, the display's ID must match (§1.2.2); the checksum example (§1.2.1). NAK error codes are
      internal and differ by command and model; an unsupported field reads 0xFF (§2, note). Every command's
      request, ACK and NAK layout is in §2.1.00-§2.1.FE (pages 28-310), and each command here names its
      section and page; H- and V-position by value in Annex A.2 and A.3 (pages 313-314).
  - title: "samsung-mdc (Python MDC library and CLI), connection.py"
    author: "Victor Gavro"
    url: "https://github.com/vgavro/samsung-mdc/blob/master/samsung_mdc/connection.py"
    note: >
      BSD-3-Clause, read as corroboration only, for the secured mode that Ver. 15.0 does not describe: on
      connecting the display sends MDCSTART<<TLS>>, the client then makes a TLS handshake on the same
      connection without checking the display's certificate, sends the 4-digit PIN, and the display answers
      MDCAUTH<<PASS>> or MDCAUTH<<FAIL:xxxx>>. Also corroborates the frame layout and checksum.

settings:
  display_id: { type: int, min: 0, max: 253, default: 0, label: Display ID,
    description: "The ID set on the display (its MDC or Device ID); commands without a display_id go to it." }
  chain_ids: { type: string, max_length: 1024, pattern: "^([0-9]{1,3}(,[0-9]{1,3})*)?$", default: "", label: Chained display IDs,
    description: "IDs of further displays reached through this one's RS-232C loop-out (or the same serial port), comma-separated, such as 2,3,4; each is polled and kept in state." }
  link: { type: enum, values: [tcp, serial], default: tcp, label: Link,
    description: "tcp reaches the display over its network port (1515); serial uses an RS-232C port of the host." }
  pin: { type: string, secret: true, pattern: "^([0-9]{4})?$", default: "", label: Secured protocol PIN,
    description: "The 4-digit PIN of the display's secured MDC mode (TLS); empty for a display with the secured protocol off." }
  mac_address: { type: string, pattern: "^(([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2})?$", default: "", label: MAC address,
    description: "The display's MAC address, such as 00:11:22:33:44:55, for waking it with Wake-on-LAN; empty sends none." }
  wol_address: { type: string, pattern: "^([0-9]{1,3}\\\\.){3}[0-9]{1,3}$", default: "255.255.255.255", label: Wake-on-LAN address,
    description: "Where the Wake-on-LAN packet is broadcast, such as the display's subnet broadcast address 192.168.0.255." }
  poll_interval_s: { type: int, min: 0, max: 3600, default: 5, label: Status poll interval,
    description: "Seconds between reads of power, input, volume, mute and the error status; 0 reads them only on connecting." }
  refresh_interval_s: { type: int, min: 0, max: 86400, default: 300, label: Full refresh interval,
    description: "Seconds between reads of every setting the display supports; 0 reads them only on connecting and on refresh." }
  serial_port: { type: string, max_length: 256, label: Serial port,
    description: "With link serial, the host's serial port wired to the display's RS-232C IN: COM3 on Windows, /dev/ttyUSB0 on Linux, /dev/cu.usbserial-1420 on macOS." }
  baud_rate: { type: int, min: 9600, max: 9600, default: 9600, label: Baud rate, description: "With link serial, MDC runs at 9600 bit/s." }
  data_bits: { type: int, min: 8, max: 8, default: 8, label: Data bits, description: "With link serial, MDC uses 8 data bits." }
  parity: { type: enum, values: [none], default: none, label: Parity, description: "With link serial, MDC uses no parity." }
  stop_bits: { type: int, min: 1, max: 1, default: 1, label: Stop bits, description: "With link serial, MDC uses 1 stop bit." }
  flow_control: { type: enum, values: [none], default: none, label: Flow control, description: "With link serial, MDC uses no flow control." }

ports:
  - { port: 1515, protocol: tcp, role: control, when: "link tcp", note: "MDC over RJ45; with a PIN the same port carries the secured (TLS) mode after a plain-text greeting" }
  - { port: null, protocol: serial, role: control, setting: serial_port, when: "link serial", note: "RS-232C, D-sub 9-pin or the display's stereo jack, wired to a serial port of the host" }
  - { port: 9, protocol: udp, role: control, when: "mac_address set", note: "Wake-on-LAN packet to wol_address and to the display, when it is off" }
"""

DISPLAY_PARAM = ("{ type: int, min: 0, max: 254, label: Display ID, description: \"The display to address, by its ID; "
                 "absent addresses the display_id setting's, 254 every display on the chain at once (unacknowledged).\" }")


def q(s):
    return json.dumps(s, ensure_ascii=False)


def hexb(v):
    return f"0x{v:02X}"


def where(d):
    code = f"{hexb(d.cmd)}" + (f"/{hexb(d.sub)}" if d.sub is not None else "")
    if d.section.startswith("A."):
        return f"{code}, Annex {d.section} p.{d.page}"
    return f"{code}, §{d.section} p.{d.page}"


def param_yaml(f):
    label = f.label or label_of(f.name)
    desc = f.desc
    if f.kind in ("u8", "u16", "i8"):
        body = f"type: int, min: {f.lo}, max: {f.hi}, required: true"
    elif f.kind == "enum":
        vals = ", ".join(q(v) for _, v, settable in f.values if settable)
        body = f"type: enum, values: [{vals}], required: true"
    elif f.kind in ("text", "ptext", "fixed"):
        body = f"type: string, max_length: {f.n}, required: true"
    elif f.kind == "ip4":
        body = "type: string, pattern: \"^([0-9]{1,3}\\\\.){3}[0-9]{1,3}$\", required: true"
    elif f.kind == "hex":
        body = f"type: string, pattern: \"^([0-9A-Fa-f]{{2}})*$\", max_length: {(f.n or 255) * 2}"
        body += ", default: \"\"" if f.name == "query" else ", required: true"
    else:
        raise ValueError(f"{f.name}: a {f.kind} field cannot be a parameter")
    secret = ", secret: true" if f.name == "password" else ""
    return f"{{ {body}{secret}, label: {q(label)}, description: {q(desc)} }}"


def get_name(d):
    return d.get_name or f"get_{d.key}"


def set_name(d):
    return d.set_name or f"set_{d.key}"


def commands_yaml():
    out, names = [], []

    def add(name, summary, params, returns):
        assert name not in names, f"duplicate command {name}"
        names.append(name)
        out.append(f"  {name}:")
        out.append(f"    summary: {q(summary)}")
        if params:
            out.append("    params:")
            for p, y in params:
                out.append(f"      {p}: {y}")
        out.append(f"    returns: {returns}")

    for d in DEFS:
        if d.get:
            summary = d.get_summary or f"Read {d.summary} ({where(d)})"
            add(get_name(d), summary, [("display_id", DISPLAY_PARAM)] + [(f.name, param_yaml(f)) for f in d.get_args], "value")
        if d.set is not None:
            if d.set_summary:
                summary = d.set_summary if "§" in d.set_summary else f"{d.set_summary} ({where(d)})"
            elif d.summary[0].isupper():
                summary = f"{d.summary} ({where(d)})"
            else:
                summary = f"Set {d.summary} ({where(d)})" + (". The reply is read back into state" if d.state else "")
            params = [("display_id", DISPLAY_PARAM)] + [(f.name, param_yaml(f)) for f in d.set if f.is_value]
            add(set_name(d), summary, params, "ack")
    add("wake_on_lan", "Send a Wake-on-LAN packet to power on a display that is off (needs the mac_address setting)", [], "none")
    add("refresh", "Read every setting of every display again, including those it refused before", [], "ack")
    add("send_raw", "Send any MDC command and return the reply as {ack, command, data}, data as hex", [
        ("display_id", DISPLAY_PARAM),
        ("command", "{ type: int, min: 0, max: 255, required: true, label: Command, description: \"The command byte, such as 17 (0x11) for power.\" }"),
        ("data", "{ type: string, pattern: \"^([0-9A-Fa-f]{2})*$\", max_length: 510, default: \"\", label: Data, description: \"The data bytes after the length, as hex, sub-command first where there is one; empty for a plain read.\" }"),
    ], "value")
    return out, names


def state_type(f):
    return {"u8": "int", "u16": "int", "i8": "int", "enum": "string", "text": "string", "fixed": "string",
            "ptext": "string", "ip4": "string", "hex": "string", "list": "array"}[f.kind]


def state_desc(f):
    s = f.desc
    if f.kind == "enum":
        vals = [v for _, v, _ in f.values]
        if len(vals) <= 12:
            s = s.rstrip(".") + " (" + ", ".join(vals) + ")."
    return s


def single_value(d):
    """Whether a reply writes one value at the state key itself."""
    if d.flat:
        return False
    counts = [len([f for f in l if f.is_value]) for l in d.ack]
    return bool(counts) and max(counts) == 1 and all(
        f.kind != "flags" for l in d.ack for f in l if f.is_value)


def state_yaml():
    seen = {}
    for d in DEFS:
        if not d.state:
            continue
        base = d.state.replace("{index}", "*")
        for layout in d.ack:
            for f in (f for f in layout if f.is_value):
                if d.flat:
                    path = f"displays.*.{f.name}"
                elif single_value(d):
                    path = f"displays.*.{base}"
                else:
                    path = f"displays.*.{base}.{f.name}"
                if f.kind == "flags":
                    for _, bit in f.bits:
                        seen.setdefault(f"{path}.{bit}", ("bool", None, f"{f.desc.rstrip('.')}: {bit}."))
                    continue
                t = state_type(f)
                if path in seen and seen[path][0] != t:
                    raise ValueError(f"{path}: {seen[path][0]} and {t}")
                seen.setdefault(path, (t, f.unit, state_desc(f)))
    out = []
    for path in sorted(seen):
        t, unit, desc = seen[path]
        u = f", unit: {q(unit)}" if unit else ""
        out.append(f"  {path}: {{ type: {t}{u}, description: {q(desc)} }}")
    return out


QUIRKS = """quirks:
  - models: [all]
    severity: warning
    text: >
      Nothing is pushed: the core reads status (0x00), the error status (0x0D) and power every poll_interval_s,
      and every other setting the display answers every refresh_interval_s and on connecting, one request at a
      time. A read the display answers with NAK, or leaves unanswered twice, is a command its model lacks
      (support depends on the model throughout the document): it is not read again until the device is reopened
      or refreshed, and the command still fails with device_error when sent. A value read as 0xFF where the
      document gives a smaller range means not supported or invalid (§2 note) and leaves the state without a
      value; a code outside a command's table is kept as its hex text, such as 0x5f.
  - models: [all]
    severity: info
    text: >
      Opened for commands only (monitor false), nothing is read on connecting and nothing is polled; a power read
      (0x11) every 10 s while idle shows the display is there, and what replies carry still reaches state.
  - models: [all]
    severity: warning
    text: >
      Power on over the network: §2.1.11 says a display that is off takes power on only by Wake-on-LAN (on some
      models only while Network Standby is off, on the rest always), and that after power on over RJ45 the
      client must reconnect after 10 s. set_power on to a display that cannot be reached sends Wake-on-LAN to
      wol_address and to the display's address on UDP 9 when mac_address is set, and is reported unverified;
      after a power on acknowledged over TCP the core closes the connection and reconnects 10 s later. Power on
      and off are sent up to three times, 2 s apart, until acknowledged, as the section asks.
  - models: [all]
    severity: warning
    text: >
      The secured protocol (TLS with a 4-digit PIN) of newer firmware is not in Ver. 15.0; its handshake is
      taken from a BSD-licensed client library and needs checking on a display: with pin set, the core expects
      the plain-text greeting MDCSTART<<TLS>> on connecting, makes a TLS handshake on the same connection without
      checking the display's self-signed certificate, sends the PIN, and takes MDCAUTH<<PASS>> as success.
      MDCAUTH<<FAIL:xxxx>> is a refused PIN: the device reports unauthorized and does not try again until the
      settings change. A display that greets with MDCSTART<<TLS>> while no PIN is set is reported unauthorized
      the same way. The core speaks TLS 1.2 and 1.3 only; a display that offers only an older TLS fails the
      handshake.
  - models: [all]
    severity: info
    text: >
      Addressing (§1.1, §1.2): over RS-232C each display on a chain has a unique ID from 0 to 253 and the
      host's port reaches them all; over RJ45 the display answers only its own ID, and displays chained to its
      RS-232C OUT are reached through it by theirs. chain_ids names those further displays: each is polled and
      kept in state under its ID, and commands reach one with display_id. display_id 254 (0xFE) addresses every
      display at once; they act but none acknowledges, so the command is reported unverified.
  - models: [all]
    severity: info
    text: >
      NAK carries one error byte whose meaning the document leaves internal and model-dependent (§2 note 2): a
      NAK fails the command with device_error, the byte in its code as hex. A reply is matched to the request in
      flight by its command byte and the ID; a sub-command echo that differs from the request (the document's
      menu size reply prints 0x83 for 0x85) is still taken as its reply.
  - models: [all]
    severity: critical
    text: >
      These cannot be undone live: set_power reboot and reset_settings restart or reset the display;
      set_hdmi_uhd_color, set_uhd_output, set_live_mode, set_dynamic_range_extension and set_uniformity make the
      display restart by itself when the value changes; set_network, set_ip_mode and set_wifi_access_point move
      the display to another network address or network, and set_mdc_connection can make it stop taking MDC on
      this link; set_auto_id can change the display's ID so it no longer answers this device's; delete_holiday
      and delete_all_holidays remove holidays; start_led_auto_id renumbers every LED cabinet.
  - models: [all]
    severity: info
    text: >
      Several commands have two or three layouts told apart only by their data length, and the document does not
      say which models use which: the timers (13 values, or 15 with separate on and off repeat days), Maintenance
      Control (0x15 or 0x19 data bytes), Sound Control (five or seven equalizer bands), the multi-screen command
      (three types) and the safety screen timer (repeat or interval). Replies are decoded by their length; each
      set layout is its own command (set_timer1 and set_timer1_separate, set_multi_screen, set_multi_screen_4 and
      set_multi_screen_quad, set_safety_screen and set_safety_screen_interval), and a display refuses the one its
      model does not take.
  - models: [all]
    severity: info
    text: >
      Values the document leaves open, taken as written and listed in VERIFICATION.md: the display status
      temperature byte is read as signed (two's complement), 0xFF being taken as invalid rather than -1; the panel
      on time is a counter rising every 10 minutes, unit not stated; color temperature codes legacy_0 to legacy_10
      cover 5000 K to 15000 K in steps the document does not give; the holiday index base is not stated; the
      Timer6 and Timer7 sections label their holiday choices "Timer1"; two-byte values whose byte order a section
      does not state are sent high byte first, as the sections that state it do.
  - models: [led-signage]
    severity: info
    text: >
      The LED white balance, color correction, seam correction and gradation commands (0xD0 sub-commands 0x92,
      0x93, 0x97, 0x9B, 0x9C, 0x9F and 0xA2) pack module, block, position and color into bit fields that the
      document prints illegibly, so their data is given and returned as hex bytes, laid out as each section shows,
      rather than as named values. Their readings are not polled: get_module_wb and the others read one module,
      block or position at a time with query.
  - models: [all]
    severity: info
    text: >
      Over RJ45 the display's own ID still applies: a display_id that does not match the display's gives no
      answer at all, which shows as timeouts (§1.2.2). Some models stop taking MDC on RS-232C once
      set_mdc_connection chooses RJ45 (§2.1.1D). The Wi-Fi password of set_wifi_access_point travels in clear
      text, as the protocol has it.
"""


def wrap_list(cmds):
    lines, line = [], "      ["
    for i, c in enumerate(cmds):
        piece = c + (", " if i < len(cmds) - 1 else "]")
        if len(line) + len(piece) > 118:
            lines.append(line.rstrip())
            line = "      "
        line += piece
    lines.append(line)
    return "\n".join(lines)


def models_yaml(all_cmds, led_cmds):
    lcd = [c for c in all_cmds if c not in led_cmds]
    return f"""models:
  - id: lcd-signage
    name: "Samsung LCD signage and large format displays (SMART Signage QM, QB, QH, QE, OM, PM, DM, DB, DH, UE, ME, MD and SyncMaster LFD series)"
    supports:
{wrap_list(lcd)}
    verification: none
    notes: >
      Every command of the protocol except the LED cabinet features; which a given model answers depends on the
      model (see the quirks).
  - id: led-signage
    name: "Samsung LED signage (The Wall, IF, IW and IE series cabinets and S-Box)"
    supports:
{wrap_list(all_cmds)}
    verification: none
    notes: >
      Adds the LED Product Feature commands (0xD0), the LED dimming and location settings (0xCA 0x60-0x66) and the
      cabinet software versions (0x1B 0xA4).
"""


def spec_text():
    cmds, names = commands_yaml()
    led = set()
    for d in DEFS:
        if d.led:
            if d.get:
                led.add(get_name(d))
            if d.set is not None:
                led.add(set_name(d))
    return (HEADER + "\n" + models_yaml(names, led) + "\ncommands:\n" + "\n".join(cmds) + "\n\n" + QUIRKS
            + "\nstate:\n" + "\n".join(state_yaml()) + "\n")


# ── The Rust table ──────────────────────────────────────────────────────────

def rust_kind(f):
    k = f.kind
    if k in ("u8", "u16", "i8"):
        return f"Kind::{k.upper()} {{ min: {f.lo}, max: {f.hi} }}"
    if k == "enum":
        return f"Kind::Enum({table_name(tuple((c, v) for c, v, _ in f.values))})"
    if k == "flags":
        return f"Kind::Flags({table_name(tuple(f.bits))})"
    if k == "text":
        return f"Kind::Text {{ max: {f.n} }}"
    if k == "fixed":
        return f"Kind::Fixed({f.n})"
    if k == "ptext":
        return f"Kind::PText {{ max: {f.n} }}"
    if k == "ip4":
        return "Kind::Ip4"
    if k == "hex":
        return f"Kind::Hex {{ max: {f.n or 255} }}"
    if k == "lit":
        return f"Kind::Lit({hexb(f.value)})"
    if k == "skip":
        return f"Kind::Skip({f.n})"
    if k == "list":
        return f"Kind::List {{ prefix: {str(f.prefix).lower()}, item: {rust_fields(f.item)} }}"
    raise ValueError(k)


def rust_fields(fields):
    return "&[" + ", ".join(f"Field {{ name: {q(f.name)}, kind: {rust_kind(f)} }}" for f in fields) + "]"


def rust_layouts(layouts):
    return "&[" + ", ".join(rust_fields(l) for l in layouts) + "]"


def opt(v):
    return f"Some({v})" if v is not None else "None"


def rust_text():
    defs = []
    for d in DEFS:
        poll = {"fast": "Poll::Fast", "slow": "Poll::Slow", "identity": "Poll::Identity", "never": "Poll::Never"}[d.poll]
        defs.append(
            "    Def {\n"
            f"        key: {q(d.key)},\n"
            f"        state: {opt(q(d.state) if d.state else None)},\n"
            f"        cmd: {hexb(d.cmd)},\n"
            f"        sub: {opt(hexb(d.sub) if d.sub is not None else None)},\n"
            f"        get: {opt(q(get_name(d)) if d.get else None)},\n"
            f"        get_args: {rust_fields(d.get_args)},\n"
            f"        set: {opt(q(set_name(d)) if d.set is not None else None)},\n"
            f"        set_fields: {rust_fields(d.set or [])},\n"
            f"        ack: {rust_layouts(d.ack or [])},\n"
            f"        set_ack: {rust_layouts(d.set_ack or [])},\n"
            f"        poll: {poll},\n"
            f"        flat: {str(d.flat).lower()},\n"
            f"        single: {str(single_value(d)).lower()},\n"
            "    },")
    body = "\n".join(defs)
    tables = "\n".join(
        "#[rustfmt::skip]\n"
        f"const {name}: &[(u8, &str)] = &[" + ", ".join(f"({hexb(c)}, {q(v)})" for c, v in t) + "];"
        for name, t in TABLES.items())
    return ("//! Generated by tools/generate_samsung_mdc.py from Samsung's Multiple Display\n"
            "//! Control Protocol Ver. 15.0. Do not edit by hand.\n\n"
            "use super::samsung_mdc::{Def, Field, Kind, Poll};\n\n"
            "#[rustfmt::skip]\n"
            "pub(crate) const DEFS: &[Def] = &[\n" + body + "\n];\n\n" + tables + "\n")


def main():
    spec = spec_text()
    rust = rust_text()
    (ROOT / "specs").mkdir(parents=True, exist_ok=True)
    (ROOT / "specs" / "samsung-mdc.yaml").write_text(spec, encoding="utf-8", newline="\n")
    out = ROOT / "crates" / "core" / "src" / "modules"
    out.mkdir(parents=True, exist_ok=True)
    (out / "samsung_mdc_table.rs").write_text(rust, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
