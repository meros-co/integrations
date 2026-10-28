//! The UPS-MIB (RFC 1628) profile: generated from UPS-MIB (RFC 1628) by
//! tools/generate_snmp_profile.py. Do not edit by hand; run the tool again.

use super::snmp_device::{Kind, Object, Profile, Root, Setter, Wire};

static E0: [(i64, &str); 4] = [
    (1, "unknown"),
    (2, "battery_normal"),
    (3, "battery_low"),
    (4, "battery_depleted"),
];
static E1: [(i64, &str); 7] = [
    (1, "other"),
    (2, "none"),
    (3, "normal"),
    (4, "bypass"),
    (5, "battery"),
    (6, "booster"),
    (7, "reducer"),
];
static E2: [(i64, &str); 6] = [
    (1, "done_pass"),
    (2, "done_warning"),
    (3, "done_error"),
    (4, "aborted"),
    (5, "in_progress"),
    (6, "no_tests_initiated"),
];
static E3: [(i64, &str); 2] = [(1, "output"), (2, "system")];
static E4: [(i64, &str); 2] = [(1, "on"), (2, "off")];
static E5: [(i64, &str); 3] = [(1, "disabled"), (2, "enabled"), (3, "muted")];
static C0: [(&[u32], &str); 24] = [
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 1], "battery_bad"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 2], "on_battery"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 3], "low_battery"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 4], "depleted_battery"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 5], "temp_bad"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 6], "input_bad"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 7], "output_bad"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 8], "output_overload"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 9], "on_bypass"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 10], "bypass_bad"),
    (
        &[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 11],
        "output_off_as_requested",
    ),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 12], "ups_off_as_requested"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 13], "charger_failed"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 14], "ups_output_off"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 15], "ups_system_off"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 16], "fan_failure"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 17], "fuse_failure"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 18], "general_fault"),
    (
        &[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 19],
        "diagnostic_test_failed",
    ),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 20], "communications_lost"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 21], "awaiting_power"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 22], "shutdown_pending"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 23], "shutdown_imminent"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 6, 3, 24], "test_in_progress"),
];
static C1: [(&[u32], &str); 5] = [
    (&[1, 3, 6, 1, 2, 1, 33, 1, 7, 7, 1], "no_tests_initiated"),
    (
        &[1, 3, 6, 1, 2, 1, 33, 1, 7, 7, 2],
        "abort_test_in_progress",
    ),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 7, 7, 3], "general_systems_test"),
    (&[1, 3, 6, 1, 2, 1, 33, 1, 7, 7, 4], "quick_battery_test"),
    (
        &[1, 3, 6, 1, 2, 1, 33, 1, 7, 7, 5],
        "deep_battery_calibration",
    ),
];

static OBJECTS: [Object; 55] = [
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 1],
        path: "ident.manufacturer",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 2],
        path: "ident.model",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 3],
        path: "ident.ups_software_version",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 4],
        path: "ident.agent_software_version",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 5],
        path: "ident.name",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 6],
        path: "ident.attached_devices",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 1],
        path: "battery.status",
        kind: Kind::Enum(&E0),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 2],
        path: "battery.seconds_on_battery",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 3],
        path: "battery.estimated_minutes_remaining",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 4],
        path: "battery.estimated_charge_remaining",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 5],
        path: "battery.voltage",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 6],
        path: "battery.current",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 2, 7],
        path: "battery.temperature",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 3, 1],
        path: "input_line_bads",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 3, 2],
        path: "input_num_lines",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 3, 3, 1, 2],
        path: "input.*.frequency",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 3, 3, 1, 3],
        path: "input.*.voltage",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 3, 3, 1, 4],
        path: "input.*.current",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 3, 3, 1, 5],
        path: "input.*.true_power",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 1],
        path: "output_source",
        kind: Kind::Enum(&E1),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 2],
        path: "output_frequency",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 3],
        path: "output_num_lines",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 4, 1, 2],
        path: "output.*.voltage",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 4, 1, 3],
        path: "output.*.current",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 4, 1, 4],
        path: "output.*.power",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 4, 4, 1, 5],
        path: "output.*.percent_load",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 5, 1],
        path: "bypass_frequency",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 5, 2],
        path: "bypass_num_lines",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 5, 3, 1, 2],
        path: "bypass.*.voltage",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 5, 3, 1, 3],
        path: "bypass.*.current",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 5, 3, 1, 4],
        path: "bypass.*.power",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 6, 1],
        path: "alarm_alarms_present",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 6, 2, 1, 2],
        path: "alarm.*.descr",
        kind: Kind::Choice(&C0),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 6, 2, 1, 3],
        path: "alarm.*.time",
        kind: Kind::Ticks,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 1],
        path: "test.id",
        kind: Kind::Choice(&C1),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 2],
        path: "test.spin_lock",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 3],
        path: "test.results_summary",
        kind: Kind::Enum(&E2),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 4],
        path: "test.results_detail",
        kind: Kind::Text,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 5],
        path: "test.start_time",
        kind: Kind::Ticks,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 6],
        path: "test.elapsed_time",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 1],
        path: "control.shutdown_type",
        kind: Kind::Enum(&E3),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 2],
        path: "control.shutdown_after_delay",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 3],
        path: "control.startup_after_delay",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 4],
        path: "control.reboot_with_duration",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 5],
        path: "control.auto_restart",
        kind: Kind::Enum(&E4),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 1],
        path: "config.input_voltage",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 2],
        path: "config.input_freq",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 3],
        path: "config.output_voltage",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 4],
        path: "config.output_freq",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 5],
        path: "config.output_va",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 6],
        path: "config.output_power",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 7],
        path: "config.low_batt_time",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 8],
        path: "config.audible_status",
        kind: Kind::Enum(&E5),
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 9],
        path: "config.low_voltage_transfer_point",
        kind: Kind::Int,
    },
    Object {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 10],
        path: "config.high_voltage_transfer_point",
        kind: Kind::Int,
    },
];

static SETTERS: [Setter; 17] = [
    Setter {
        command: "set_ident_name",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 5],
        kind: Kind::Text,
        wire: Wire::OctetString,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 1],
        lock: &[],
    },
    Setter {
        command: "set_ident_attached_devices",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 1, 6],
        kind: Kind::Text,
        wire: Wire::OctetString,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 1],
        lock: &[],
    },
    Setter {
        command: "set_test_id",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 1],
        kind: Kind::Choice(&C1),
        wire: Wire::ObjectIdentifier,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 7],
        lock: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 2],
    },
    Setter {
        command: "set_test_spin_lock",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 7, 2],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 7],
        lock: &[],
    },
    Setter {
        command: "set_control_shutdown_type",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 1],
        kind: Kind::Enum(&E3),
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 8],
        lock: &[],
    },
    Setter {
        command: "set_control_shutdown_after_delay",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 2],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 8],
        lock: &[],
    },
    Setter {
        command: "set_control_startup_after_delay",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 3],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 8],
        lock: &[],
    },
    Setter {
        command: "set_control_reboot_with_duration",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 4],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 8],
        lock: &[],
    },
    Setter {
        command: "set_control_auto_restart",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 8, 5],
        kind: Kind::Enum(&E4),
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 8],
        lock: &[],
    },
    Setter {
        command: "set_config_input_voltage",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 1],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_input_freq",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 2],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_output_voltage",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 3],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_output_freq",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 4],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_low_batt_time",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 7],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_audible_status",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 8],
        kind: Kind::Enum(&E5),
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_low_voltage_transfer_point",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 9],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
    Setter {
        command: "set_config_high_voltage_transfer_point",
        oid: &[1, 3, 6, 1, 2, 1, 33, 1, 9, 10],
        kind: Kind::Int,
        wire: Wire::Integer,
        indexes: &[],
        reread: &[1, 3, 6, 1, 2, 1, 33, 1, 9],
        lock: &[],
    },
];

static TRAPS: [(&[u32], &str); 4] = [
    (&[1, 3, 6, 1, 2, 1, 33, 2, 1], "upsTrapOnBattery"),
    (&[1, 3, 6, 1, 2, 1, 33, 2, 2], "upsTrapTestCompleted"),
    (&[1, 3, 6, 1, 2, 1, 33, 2, 3], "upsTrapAlarmEntryAdded"),
    (&[1, 3, 6, 1, 2, 1, 33, 2, 4], "upsTrapAlarmEntryRemoved"),
];

pub(crate) static PROFILE: Profile = Profile {
    roots: &[Root {
        oid: &[1, 3, 6, 1, 2, 1, 33, 1],
        models: &[],
    }],
    objects: &OBJECTS,
    setters: &SETTERS,
    traps: &TRAPS,
};
