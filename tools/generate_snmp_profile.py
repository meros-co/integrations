"""Generate an SNMP integration's spec sections and the native module's
profile from the vendor's MIB, which is not kept in this repository (its
license is the vendor's): download it, then

    python tools/generate_snmp_profile.py <spec-id> <mib file>

    python tools/generate_snmp_profile.py apc-rack-pdu powernet460.mib
    python tools/generate_snmp_profile.py apc-ups powernet460.mib
    python tools/generate_snmp_profile.py cyberpower-pdu CyberPower_MIB_v2.13.MIB
    python tools/generate_snmp_profile.py cyberpower-ups CyberPower_MIB_v2.13.MIB
    python tools/generate_snmp_profile.py ups-mib rfc1628.txt

It rewrites specs/<id>.yaml from `settings:` to `quirks:` (settings, ports,
commands and state) and each model's `supports`, keeping the spec's head and
quirks as written, and writes crates/core/src/modules/<id>_profile.rs (and
the vendor's notification names), formatted with rustfmt. Every object of
the subtrees PROFILES names becomes state, every writable one a command
named after it; names, OIDs, syntaxes, units and enumeration labels are the
MIB's facts, and no MIB text is copied. Review the diff, then run the checks.

Each number's unit and step come from the MIB: the profile's `units` table
(each object's unit and divisor as its DESCRIPTION states them), its UNITS
clause, or the DESCRIPTION's wording. A profile with `si` reports a number
counted in steps of a fraction of its unit (tenths of a volt, TimeTicks) in
that unit, as a float: the module divides it, and a command writing it takes
the unit and multiplies. Without `si` the number is reported as the agent
counts it and the state's `scale` gives the step (SPEC.md §1, Units and
scale).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# For each spec: the MIB's name and version as cited, the models, the
# subtrees walked (and for which models; `prefix` is left off object names,
# `state_prefix` put before their state paths), the notification OIDs named,
# OBJECT IDENTIFIER objects that name one of a group (`oid_choices`), and
# advisory locks written with an object (`locks`).
PROFILES = {
    "apc-rack-pdu": {
        "title": "APC Rack PDU", "mib": "PowerNet-MIB", "mib_version": "v4.6.0",
        "models": ["rack-pdu-2g", "rack-pdu"],
        "roots": [{"name": "rPDU2", "models": ["rack-pdu-2g"]}, {"name": "rPDU", "models": ["rack-pdu"]}],
        "trap_prefixes": ["1.3.6.1.4.1.318.0."], "traps_module": "powernet_traps",
        "si": True,
    },
    "apc-ups": {
        "title": "APC UPS", "mib": "PowerNet-MIB", "mib_version": "v4.6.0",
        "models": ["ups"],
        "roots": [{"name": "ups", "models": ["ups"]}],
        "trap_prefixes": ["1.3.6.1.4.1.318.0."], "traps_module": "powernet_traps",
        "si": True,
    },
    "cyberpower-pdu": {
        "title": "CyberPower PDU", "mib": "CPS-MIB", "mib_version": "v2.13",
        "models": ["epdu2", "epdu"],
        "roots": [{"name": "ePDU2", "models": ["epdu2"]}, {"name": "ePDU", "models": ["epdu"]},
                  {"name": "environmentSensor", "prefix": "envir", "state_prefix": "environment",
                   "models": ["epdu2", "epdu"]},
                  {"name": "environmentSensor2", "prefix": "envir2", "state_prefix": "environment2",
                   "models": ["epdu2", "epdu"]}],
        "trap_prefixes": ["1.3.6.1.4.1.3808.0."], "traps_module": "cps_traps",
        "si": True, "units": "CPS",
    },
    "cyberpower-ups": {
        "title": "CyberPower UPS", "mib": "CPS-MIB", "mib_version": "v2.13",
        "models": ["ups"],
        "roots": [{"name": "ups", "models": ["ups"]},
                  {"name": "environmentSensor", "prefix": "envir", "state_prefix": "environment", "models": ["ups"]},
                  {"name": "environmentSensor2", "prefix": "envir2", "state_prefix": "environment2",
                   "models": ["ups"]}],
        "trap_prefixes": ["1.3.6.1.4.1.3808.0."], "traps_module": "cps_traps",
        "si": True, "units": "CPS",
    },
    "ups-mib": {
        "title": "UPS-MIB (RFC 1628)", "mib": "UPS-MIB", "mib_version": "(RFC 1628)",
        "models": ["ups"],
        "roots": [{"name": "upsObjects", "prefix": "ups", "models": ["ups"]}],
        "trap_prefixes": ["1.3.6.1.2.1.33.2."],
        "oid_choices": {"upsTestId": "upsWellKnownTests", "upsAlarmDescr": "upsWellKnownAlarms"},
        "locks": {"upsTestId": "upsTestSpinLock"},
        "si": True,
    },
}

# ── Reading a MIB ───────────────────────────────────────────────────────────


ROOTS = {
    'iso': [1], 'org': [1, 3], 'dod': [1, 3, 6], 'internet': [1, 3, 6, 1],
    'mgmt': [1, 3, 6, 1, 2], 'mib-2': [1, 3, 6, 1, 2, 1], 'private': [1, 3, 6, 1, 4],
    'enterprises': [1, 3, 6, 1, 4, 1], 'experimental': [1, 3, 6, 1, 3],
    'snmpV2': [1, 3, 6, 1, 6], 'system': [1, 3, 6, 1, 2, 1, 1],
    'transmission': [1, 3, 6, 1, 2, 1, 10],
}


def strip_comments(text):
    out = []
    for line in text.splitlines():
        # remove -- comments outside strings
        res, in_str, i = '', False, 0
        while i < len(line):
            c = line[i]
            if c == '"':
                in_str = not in_str
                res += c
            elif not in_str and line.startswith('--', i):
                # comment ends at next -- or end of line
                j = line.find('--', i + 2)
                if j < 0:
                    break
                i = j + 2
                continue
            else:
                res += c
            i += 1
        out.append(res)
    return '\n'.join(out)


def parse_text(text):
    text = strip_comments(text)
    k = text.find('DEFINITIONS ::= BEGIN')
    if k > 0:
        text = text[k + 21:]
    text = re.sub(r'IMPORTS.*?;', '', text, flags=re.S)
    # blank out string contents but keep DESCRIPTION for length only
    defs = {}
    tcs = {}
    # textual conventions: Name ::= TEXTUAL-CONVENTION ... SYNTAX X
    for m in re.finditer(r'(\w[\w-]*)\s*::=\s*TEXTUAL-CONVENTION(.*?)SYNTAX\s+(.+?)(?=\n\s*\n|\n\w[\w-]*\s*::=|\Z)', text, re.S):
        tcs[m.group(1)] = m.group(3).strip()
    for m in re.finditer(r'^\s*(\w[\w-]*)\s*::=\s*(?:\[APPLICATION \d+\]\s*(?:IMPLICIT\s*)?)?(DisplayString|OCTET STRING|INTEGER|Integer32|Unsigned32|Gauge32|Gauge|Counter32|Counter|TimeTicks)([^\n]*)', text, re.M):
        tcs.setdefault(m.group(1), (m.group(2) + m.group(3)).strip())
    pat = re.compile(
        r'(\w[\w-]*)\s+(OBJECT IDENTIFIER|OBJECT-TYPE|OBJECT-IDENTITY|MODULE-IDENTITY|NOTIFICATION-TYPE|TRAP-TYPE|OBJECT-GROUP|NOTIFICATION-GROUP|MODULE-COMPLIANCE)\b(.*?)::=\s*(\{[^}]*\}|\d+)',
        re.S)
    for m in pat.finditer(text):
        name, kind, body, val = m.group(1), m.group(2), m.group(3), m.group(4)
        if name in ('SYNTAX', 'MACRO'):
            continue
        d = {'name': name, 'kind': kind}
        if val.startswith('{'):
            parts = val.strip('{} \n').split()
            d['parent'] = parts[0]
            subs = []
            for p in parts[1:]:
                mm = re.match(r'(?:\w[\w-]*\()?(\d+)\)?', p)
                if mm:
                    subs.append(int(mm.group(1)))
            d['subs'] = subs
        else:
            d['trapnum'] = int(val)
        if kind == 'TRAP-TYPE':
            e = re.search(r'ENTERPRISE\s+(\w[\w-]*)', body)
            d['enterprise'] = e.group(1) if e else None
            v = re.search(r'VARIABLES\s*\{([^}]*)\}', body)
            d['variables'] = [x.strip() for x in v.group(1).split(',')] if v else []
        if kind == 'NOTIFICATION-TYPE':
            v = re.search(r'OBJECTS\s*\{([^}]*)\}', body)
            d['variables'] = [x.strip() for x in v.group(1).split(',')] if v else []
        if kind == 'OBJECT-TYPE':
            s = re.search(r'SYNTAX\s+(.*?)\s+(?:UNITS|MAX-ACCESS|ACCESS)\b', body, re.S)
            d['syntax'] = re.sub(r'\s+', ' ', s.group(1)).strip() if s else ''
            st = re.search(r'STATUS\s+([\w-]+)', body)
            d['status'] = st.group(1) if st else ''
            a = re.search(r'(?:MAX-ACCESS|ACCESS)\s+([\w-]+)', body)
            d['access'] = a.group(1) if a else ''
            u = re.search(r'UNITS\s+"([^"]*)"', body)
            if u:
                d['units'] = u.group(1)
            i = re.search(r'INDEX\s*\{([^}]*)\}', body)
            if i:
                d['index'] = [x.strip().replace('IMPLIED ', '') for x in i.group(1).split(',')]
            au = re.search(r'AUGMENTS\s*\{([^}]*)\}', body)
            if au:
                d['augments'] = au.group(1).strip()
            ds = re.search(r'DESCRIPTION\s*"(.*?)"', body, re.S)
            d['description'] = re.sub(r'\s+', ' ', ds.group(1)).strip() if ds else ''
        defs.setdefault(name, d)
    # resolve OIDs
    oids = {}

    def resolve(n, depth=0):
        if n in ROOTS:
            return ROOTS[n]
        if n in oids:
            return oids[n]
        d = defs.get(n)
        if d is None or depth > 60 or 'parent' not in d:
            return None
        p = resolve(d['parent'], depth + 1)
        if p is None:
            return None
        oids[n] = p + d['subs']
        return oids[n]

    for n, d in defs.items():
        if 'parent' in d:
            o = resolve(n)
            if o:
                d['oid'] = '.'.join(map(str, o))
        elif d['kind'] == 'TRAP-TYPE' and d.get('enterprise'):
            e = resolve(d['enterprise'])
            if e:
                # RFC 3584 §3.1: enterprise.0.specific
                d['oid'] = '.'.join(map(str, e + [0, d['trapnum']]))
    # enum and base type
    for d in defs.values():
        if d['kind'] != 'OBJECT-TYPE':
            continue
        syn = d['syntax']
        base = syn
        seen = 0
        while True:
            w = re.match(r'(\w[\w-]*)', base)
            if w and w.group(1) in tcs and seen < 5:
                tc = tcs[w.group(1)]
                rest = base[len(w.group(1)):]
                base = tc + (rest if '{' not in tc else '')
                seen += 1
            else:
                break
        en = re.match(r'(?:INTEGER|Integer32)?\s*\{(.*)\}', base, re.S)
        if en and 'INTEGER' in (base.split('{')[0] or 'INTEGER') or (en and base.strip().startswith('{')):
            items = re.findall(r'([\w-]+)\s*\(\s*(-?\d+)\s*\)', en.group(1))
            d['enum'] = [(k, int(v)) for k, v in items]
            d['base'] = 'enum'
        elif base.startswith('SEQUENCE OF'):
            d['base'] = 'table'
        elif re.match(r'^[A-Z]\w*Entry$', base) or (re.match(r'^\w+$', base) and base not in tcs and base[0].isupper() and base.endswith('Entry')):
            d['base'] = 'entry'
        elif re.match(r'(DisplayString|OCTET STRING|SnmpAdminString|PhysAddress|MacAddress|DateAndTime|InetAddress)', base):
            d['base'] = 'string'
            if re.match(r'(PhysAddress|MacAddress)', syn):
                d['base'] = 'mac'
        elif re.match(r'(IpAddress|NetworkAddress)', base):
            d['base'] = 'ip'
        elif re.match(r'(TimeTicks|TimeStamp)', base):
            d['base'] = 'timeticks'
        elif re.match(r'(OBJECT IDENTIFIER|AutonomousType)', base):
            d['base'] = 'oid'
        elif re.match(r'(Counter64)', base):
            d['base'] = 'counter64'
        elif re.match(r'(Counter|Counter32)', base):
            d['base'] = 'counter'
        elif re.match(r'(INTEGER|Integer32|Unsigned32|Gauge32|Gauge|NonNegativeInteger|PositiveInteger|TruthValue|TestAndIncr|TimeInterval)', base):
            if base.startswith('TruthValue'):
                d['enum'] = [('true', 1), ('false', 2)]
                d['base'] = 'enum'
            else:
                d['base'] = 'int'
                r = re.search(r'\(\s*(-?\d+)\s*\.\.\s*(-?\d+)\s*\)', base)
                if r:
                    d['range'] = (int(r.group(1)), int(r.group(2)))
        else:
            d['base'] = 'other:' + base[:40]
        if syn.startswith('TruthValue') and 'enum' not in d:
            d['enum'] = [('true', 1), ('false', 2)]
            d['base'] = 'enum'
        sz = re.search(r'SIZE\s*\(\s*(\d+)(?:\s*\.\.\s*(\d+))?\s*\)', base)
        if sz and d.get('base') in ('string',):
            d['size'] = (int(sz.group(1)), int(sz.group(2) or sz.group(1)))
    return defs


# ── Profiles ────────────────────────────────────────────────────────────────

INT_MIN, INT_MAX = -2147483648, 2147483647


def words(name):
    return re.findall(r'[A-Z]+(?=[A-Z][a-z]|\d|$)|[A-Z]?[a-z]+|\d+|[A-Z]+', name)


def snake_label(label):
    """An enumeration label in snake case (same as the Rust engine's)."""
    out = ''
    for i, c in enumerate(label):
        if c in '- ':
            out += '_'
        elif c.isupper():
            prev = label[i - 1] if i else None
            nxt = label[i + 1] if i + 1 < len(label) else None
            boundary = (prev is not None and (prev.islower() or prev.isdigit())) or (
                prev is not None and prev.isupper() and nxt is not None and nxt.islower())
            if boundary and not out.endswith('_'):
                out += '_'
            out += c.lower()
        else:
            out += c
    return re.sub(r'_+', '_', out).strip('_')


def strip_words(name, prefix):
    """Words of `name` after the words it shares with `prefix`."""
    a, b = words(name), words(prefix)
    i = 0
    while i < len(a) and i < len(b) and a[i].lower() == b[i].lower():
        i += 1
    rest = a[i:] or a[-1:]
    return rest


def snake_words(ws):
    return '_'.join(w.lower() for w in ws)


UNIT_PATTERNS = [
    (r'tenths of (?:amps|amperes|a\b)', ('A', 10)),
    (r'hundredths of (?:amps|amperes)', ('A', 100)),
    (r'tenths of (?:volts|vac|vdc|v\b)', ('V', 10)),
    (r'hundredths of (?:a )?volts?', ('V', 100)),
    (r'tenths of (?:hertz|hz)', ('Hz', 10)),
    (r'hundredths of (?:kilowatts|kw\b)', ('kW', 100)),
    (r'tenths of (?:kilowatts|kw\b)', ('kW', 10)),
    (r'hundredths of (?:kilovoltamps|kilovolt-amps|kva)', ('kVA', 100)),
    (r'tenths of (?:kilowatt[- ]hours|kwh)', ('kWh', 10)),
    (r'hundredths of (?:kilowatt[- ]hours|kwh)', ('kWh', 100)),
    (r'tenths of (?:degrees? )?(?:celsius|c\b)', ('°C', 10)),
    (r'tenths of (?:degrees? )?(?:fahrenheit|f\b)', ('°F', 10)),
    (r'tenths of (?:a )?percent', ('%', 10)),
    (r'tenths of (?:watts|w\b)', ('W', 10)),
    (r'in hundredths\b', (None, 100)),
    (r'\b(?:measured )?in (?:volts|vac)\b', ('V', 1)),
    (r'\b(?:measured )?in (?:amps|amperes)\b', ('A', 1)),
    (r'\b(?:measured )?in watts\b', ('W', 1)),
    (r'\b(?:measured )?in (?:hertz|hz)\b', ('Hz', 1)),
    (r'\b(?:measured )?in seconds\b|\bdelay, in seconds\b', ('s', 1)),
    (r'\b(?:measured )?in minutes\b', ('min', 1)),
    (r'\bin percent\b', ('%', 1)),
    (r'\bin (?:degrees )?celsius\b', ('°C', 1)),
    (r'\bin (?:degrees )?fahrenheit\b', ('°F', 1)),
]

# A UNITS clause's words, as unit symbols; a number before them is the step
# ("0.1 Hertz").
UNIT_WORDS = {'seconds': 's', 'second': 's', 'minutes': 'min', 'percent': '%', 'volts': 'V',
              'rms volts': 'V', 'volt dc': 'V', 'amps': 'A', 'rms amp': 'A', 'amp dc': 'A',
              'watts': 'W', 'hertz': 'Hz', 'volt-amps': 'VA', 'degrees centigrade': '°C'}

# Units and steps a MIB states in words the patterns above do not read,
# keyed by object: (unit, divisor), the number counting 1/divisor of the
# unit; a unit of None for a ratio; ('@<object>', divisor, suffix) for a
# unit named by another object's value (UNIT_FROM). CPS-MIB v2.13, from
# each object's DESCRIPTION (its UNITS clause where it has one).
UNITS = {
    "CPS": {
        # ups group
        "upsAdvanceIdentPowerRating": ("VA", 1), "upsAdvanceIdentLoadPower": ("W", 1),
        "upsAdvanceIdentCurrentRating": ("A", 10), "upsBaseBatteryAgeRecommand": ("month", 1),
        "upsBaseBatteryTimeOnBattery": ("s", 100), "upsAdvanceBatteryRunTimeRemaining": ("s", 100),
        "upsAdvanceBatteryCapacity": ("%", 1), "upsAdvanceBatteryVoltage": ("V", 10),
        "upsAdvanceBatteryTemperature": ("°C", 1), "upsAdvanceBatteryFullChargeVoltage": ("V", 10),
        "upsAdvanceBatteryCurrent": ("%", 1), "upsAdvanceBatteryLife": ("%", 100),
        "upsAdvanceInputLineVoltage": ("V", 10), "upsAdvanceInputMaxLineVoltage": ("V", 10),
        "upsAdvanceInputMinLineVoltage": ("V", 10), "upsAdvanceInputFrequency": ("Hz", 10),
        "upsAdvanceInputWatt": ("W", 1), "upsAdvanceOutputVoltage": ("V", 10),
        "upsAdvanceOutputFrequency": ("Hz", 10), "upsAdvanceOutputLoad": ("%", 1),
        "upsAdvanceOutputCurrent": ("A", 10), "upsAdvanceOutputPower": ("W", 1),
        "upsAdvanceOutputEnergy": ("kWh", 10), "upsAdvanceApparentOutputPower": ("VA", 1),
        "vaRating": ("VA", 1), "upsAdvanceConfigOutputVoltage": ("V", 1),
        "upsAdvanceConfigHighTransferVolt": ("V", 1), "upsAdvanceConfigLowTransferVolt": ("V", 1),
        "upsAdvanceConfigAlarmTimer": ("s", 100), "upsAdvanceConfigLowBatteryRunTime": ("s", 100),
        "upsAdvanceConfigReturnDelay": ("s", 100), "upsAdvanceConfigShutoffDelay": ("s", 100),
        "upsAdvanceConfigSleepDelay": ("s", 100),
        "upsAdvanceConfigMinReturnCapacity": ("%", 1), "upsAdvanceConfigRechargedCapacity": ("%", 1),
        "upsAdvanceConfigDeepDischargeProtection": ("min", 1), "upsAdvanceConfigLowBatteryThreshold": ("%", 1),
        "upsAdvanceConfigBatteryRuntimeThreshold": ("min", 1), "upsAdvanceSleepAfterDelay": ("s", 1),
        "upsIndividualOutletConfigPowerOnTime": ("s", 1), "upsIndividualOutletConfigPowerOffTime": ("s", 1),
        "upsIndividualOutletConfigRebootDuration": ("s", 1),
        "upsPhaseInputVoltage": ("V", 10), "upsPhaseInputCurrent": ("A", 10),
        "upsPhaseInputFrequency": ("Hz", 10), "upsPhaseInputPowerFactor": (None, 100),
        "upsPhaseOutputVoltage": ("V", 10), "upsPhaseOutputCurrent": ("A", 10),
        "upsPhaseOutputFrequency": ("Hz", 10), "upsPhaseOutputPowerFactor": (None, 100),
        "upsPhaseOutputPower": ("W", 1), "upsPhaseBypassVoltage": ("V", 10),
        "upsPhaseBypassCurrent": ("A", 10), "upsPhaseBypassFrequency": ("Hz", 10),
        "upsPhaseBypassPowerFactor": (None, 100), "upsTemperature": ("°C", 1),
        # ePDU2 group
        "ePDU2DeviceConfigColdstartDelay": ("s", 1),
        "ePDU2DeviceConfigCurrentLowLoadThreshold": ("A", 1),
        "ePDU2DeviceConfigCurrentNearOverloadThreshold": ("A", 1),
        "ePDU2DeviceConfigCurrentOverloadThreshold": ("A", 1),
        "ePDU2DeviceConfigPowerLowLoadThreshold": ("W", 1),
        "ePDU2DeviceConfigPowerNearOverloadThreshold": ("W", 1),
        "ePDU2DeviceConfigPowerOverloadThreshold": ("W", 1),
        "ePDU2DeviceInfoBreakerRating": ("A", 1), "ePDU2DeviceInfoPowerLimit": ("W", 1),
        "ePDU2DeviceStatusCurrentLoad": ("A", 10), "ePDU2DeviceStatusCurrentPeakLoad": ("A", 10),
        "ePDU2DeviceStatusEnergy": ("kWh", 10), "ePDU2DeviceStatusApparentPower": ("VA", 1),
        "ePDU2DeviceStatusPowerFactor": (None, 100), "ePDU2DeviceStatusPowerLoad": ("W", 1),
        "ePDU2DeviceStatusPowerPeakLoad": ("W", 1),
        "ePDU2PhaseConfigLowLoadThreshold": ("A", 1), "ePDU2PhaseConfigNearOverloadThreshold": ("A", 1),
        "ePDU2PhaseConfigOverloadThreshold": ("A", 1),
        "ePDU2PhaseStatusLoad": ("A", 10), "ePDU2PhaseStatusVoltage": ("V", 10),
        "ePDU2PhaseStatusPower": ("W", 1), "ePDU2PhaseStatusApparentPower": ("VA", 1),
        "ePDU2PhaseStatusPowerFactor": (None, 100), "ePDU2PhaseStatusPeakLoad": ("A", 10),
        "ePDU2PhaseStatusLineToLineVoltage": ("V", 10),
        "ePDU2BankConfigLowLoadThreshold": ("A", 1), "ePDU2BankConfigNearOverloadThreshold": ("A", 1),
        "ePDU2BankConfigOverloadThreshold": ("A", 1),
        "ePDU2BankStatusLoad": ("A", 10), "ePDU2BankStatusPeakLoad": ("A", 10),
        "ePDU2OutletSwitchedConfigPowerOnTime": ("s", 1), "ePDU2OutletSwitchedConfigPowerOffTime": ("s", 1),
        "ePDU2OutletSwitchedConfigRebootDuration": ("s", 1),
        "ePDU2OutletMeteredConfigLowLoadThreshold": ("W", 1),
        "ePDU2OutletMeteredConfigNearOverloadThreshold": ("W", 1),
        "ePDU2OutletMeteredConfigOverloadThreshold": ("W", 1),
        "ePDU2OutletMeteredStatusLoad": ("A", 10), "ePDU2OutletMeteredStatusActivePower": ("W", 1),
        "ePDU2OutletMeteredStatusPeakPower": ("W", 1), "ePDU2OutletMeteredStatusEnergy": ("kWh", 10),
        "ePDU2GroupTotalPower": ("W", 1), "ePDU2GroupTotalEnergy": ("kWh", 10),
        "ePDU2SourceStatusNominalFrequency": ("Hz", 10), "ePDU2SourceStatusSourceAVoltage": ("V", 10),
        "ePDU2SourceStatusSourceBVoltage": ("V", 10), "ePDU2SourceStatusSourceAFrequency": ("Hz", 10),
        "ePDU2SourceStatusSourceBFrequency": ("Hz", 10),
        # ePDU group
        "ePDUIdentDeviceLinetoLineVoltage": ("V", 1),
        "ePDULoadDevMaxPhaseLoad": ("A", 1), "ePDULoadDevMaxBankLoad": ("A", 1),
        "ePDULoadDevBankMaxLoad": ("A", 1), "ePDULoadDevMaxOutletLoad": ("A", 1),
        "ePDULoadPhaseConfigLowLoadThreshold": ("A", 1), "ePDULoadPhaseConfigNearOverloadThreshold": ("A", 1),
        "ePDULoadPhaseConfigOverloadThreshold": ("A", 1),
        "ePDULoadStatusLoad": ("A", 10), "ePDULoadStatusVoltage": ("V", 10),
        "ePDULoadStatusActivePower": ("W", 1), "ePDULoadStatusApparentPower": ("VA", 1),
        "ePDULoadStatusPowerFactor": (None, 100), "ePDULoadStatusEnergy": ("kWh", 10),
        "ePDULoadBankConfigLowLoadThreshold": ("A", 1), "ePDULoadBankConfigNearOverloadThreshold": ("A", 1),
        "ePDULoadBankConfigOverloadThreshold": ("A", 1),
        "ePDUOutletDevColdstartDelay": ("s", 1), "ePDUOutletConfigPowerOnTime": ("s", 1),
        "ePDUOutletConfigPowerOffTime": ("s", 1), "ePDUOutletConfigRebootDuration": ("s", 1),
        "ePDUOutletConfigMonitoredLowLoadThreshold": ("W", 1),
        "ePDUOutletConfigMonitoredNearOverloadThreshold": ("W", 1),
        "ePDUOutletConfigMonitoredOverloadThreshold": ("W", 1),
        "ePDUOutletStatusLoad": ("A", 10), "ePDUOutletStatusActivePower": ("W", 1),
        "ePDUOutletStatusPeakPower": ("W", 1), "ePDUOutletStatusEnergy": ("kWh", 10),
        "ePDUStatusInputVoltage": ("V", 10), "ePDUStatusInputFrequency": ("Hz", 10),
        # environmentSensor and environmentSensor2
        "envirTemperature": ("°F", 10), "envirTempHighThreshold": ("°F", 1),
        "envirTempLowThreshold": ("°F", 1), "envirTempRateOfChange": ("°F/5 min", 1),
        "envirTempHysteresis": ("°F", 1), "envirTemperatureCelsius": ("°C", 10),
        "envirTempCelsiusHighThreshold": ("°C", 1), "envirTempCelsiusLowThreshold": ("°C", 1),
        "envirTempCelsiusRateOfChange": ("°C/5 min", 1), "envirTempCelsiusHysteresis": ("°C", 1),
        "envirHumidity": ("%", 1), "envirHumidHighThreshold": ("%", 1), "envirHumidLowThreshold": ("%", 1),
        "envirHumidRateOfChange": ("%/5 min", 1), "envirHumidHysteresis": ("%", 1),
        "envir2Temperature": ("@envir2TempUnit", 100, ""),
        "envir2TempHighThreshold": ("@envir2TempUnit", 1, ""),
        "envir2TempLowThreshold": ("@envir2TempUnit", 1, ""),
        "envir2TempRateOfChange": ("@envir2TempUnit", 1, "/5 min"),
        "envir2TempHysteresis": ("@envir2TempUnit", 1, ""),
        "envir2Humidity": ("%", 100), "envir2HumidHighThreshold": ("%", 1), "envir2HumidLowThreshold": ("%", 1),
        "envir2HumidRateOfChange": ("%/5 min", 1), "envir2HumidHysteresis": ("%", 1),
    },
}

# The unit each value of an object naming a unit means.
UNIT_FROM = {"envir2TempUnit": {"celsius": "°C", "fahrenheit": "°F"}}


def unit_of(o, cfg=None):
    """An object's (unit, divisor): the number counts 1/divisor of the unit;
    (None, 1) when the MIB gives none."""
    known = UNITS.get((cfg or {}).get('units'), {}).get(o['name'])
    if known:
        return known
    if o.get('units'):
        m = re.match(r'^0\.(0*)1\s+(.*)$', o['units'])
        words_, divisor = (m.group(2), 10 ** (len(m.group(1)) + 1)) if m else (o['units'], 1)
        return UNIT_WORDS.get(words_.lower(), words_), divisor
    if o.get('base') == 'timeticks' or o.get('syntax', '').startswith('TimeInterval'):
        return 's', 100
    d = o.get('description', '').lower()
    for pat, unit in UNIT_PATTERNS:
        if re.search(pat, d):
            return unit
    return None, 1


def generate(d, cfg):
    for v in d.values():
        if v['kind'] == 'OBJECT-TYPE':
            p = d.get(v.get('parent'))
            if p is not None and p.get('syntax', '').startswith('SEQUENCE OF'):
                v['base'] = 'entry'
    objects, setters, paths = [], [], {}
    supports = {m: set() for m in cfg['models']}
    state = {}
    commands = {}
    for root in cfg['roots']:
        r = d[root['name']]
        ro = r['oid']
        prefix = root.get('prefix', root['name'])
        items = sorted((v for v in d.values() if v.get('oid', '').startswith(ro + '.') and v['kind'] == 'OBJECT-TYPE'),
                       key=lambda v: [int(x) for x in v['oid'].split('.')])
        for o in items:
            if o.get('base') in ('table', 'entry') or o.get('access') in ('not-accessible', 'accessible-for-notify'):
                continue
            if o.get('status') in ('obsolete',):
                continue
            parent = d[o['parent']]
            if parent.get('base') == 'entry':
                entry = parent
                base_name = entry['name'][:-5] if entry['name'].endswith('Entry') else entry['name']
                idx = entry.get('index') or (d[entry['augments']].get('index') if entry.get('augments') in d else None) or []
                if o['name'] in idx:
                    continue
                if any(i not in d or d[i].get('base') not in ('int', 'enum') for i in idx) or not idx:
                    # indexes that are not integers (strings, addresses) are not named
                    continue
                table = snake_words(strip_words(base_name, prefix))
                field = snake_words(strip_words(o['name'], base_name))
                path = table + '.*' * len(idx) + '.' + field
                index_params = []
                for i in idx:
                    pn = snake_words(strip_words(i, base_name))
                    if pn in index_params:
                        pn = snake_words(strip_words(i, prefix))
                    index_params.append(pn)
                reread = entry['oid']
            else:
                idx, index_params = [], []
                if o['parent'] == root['name']:
                    path = snake_words(strip_words(o['name'], prefix))
                    reread = o['oid']
                else:
                    group = snake_words(strip_words(o['parent'], prefix))
                    field = snake_words(strip_words(o['name'], o['parent']))
                    path = group + '.' + field
                    reread = parent['oid']
            if root.get('state_prefix'):
                path = root['state_prefix'] + '.' + path
            if path.split('.')[0] in ('system', 'engine', 'traps', 'version', 'local_engine_id'):
                path = snake_words(words(prefix)) + '_' + path
            if path in paths:
                path = path.rsplit('.', 1)[0] + '.' + snake_words(words(o['name']))
            if path in paths:
                print('two objects named', path, o['name'], paths[path], file=sys.stderr)
                continue
            paths[path] = o['name']
            base = o.get('base')
            if base == 'enum':
                kind = 'enum'
            elif base in ('int', 'counter', 'counter64'):
                kind = 'int'
            elif base == 'timeticks':
                kind = 'ticks'
            elif base == 'mac':
                kind = 'mac'
            elif base == 'ip':
                kind = 'ip'
            elif base == 'oid' and o['name'] in cfg.get('oid_choices', {}):
                kind = 'choice'
            elif base == 'oid':
                kind = 'oid'
            elif base == 'string':
                kind = 'text'
            else:
                print('left out (syntax not read):', o['name'], base, file=sys.stderr)
                continue
            labels = [(v, snake_label(k)) for k, v in o.get('enum', [])]
            if kind == 'choice':
                group = cfg['oid_choices'][o['name']]
                kids = sorted((v for v in d.values() if v.get('parent') == group and 'oid' in v),
                              key=lambda v: v['subs'])
                ws = [words(v['name']) for v in kids]
                common = 0
                while all(len(w) > common + 1 and w[common] == ws[0][common] for w in ws):
                    common += 1
                labels = [(v['oid'], snake_words(w[common:])) for v, w in zip(kids, ws)]
            known = unit_of(o, cfg) if kind in ('int', 'ticks') else (None, 1)
            unit, divisor = known[0], known[1]
            unit_from = None
            if unit and unit.startswith('@'):
                unit_from = (unit[1:], known[2])
                unit = None
            if cfg.get('si') and divisor > 1:
                # Reported in the unit: the module divides.
                kind = 'scaled'
            objects.append({'oid': o['oid'], 'path': path, 'kind': kind, 'labels': labels, 'divisor': divisor})
            stype = {'enum': 'string', 'int': 'int', 'ticks': 'int', 'scaled': 'float'}.get(kind, 'string')
            desc = f"{o['name']} ({cfg['mib']})"
            if kind in ('enum', 'choice') and labels:
                desc += ': ' + ', '.join(l for _, l in labels)
            elif kind == 'mac':
                desc += ', in hex'
            sd = {'type': stype}
            if unit:
                sd['unit'] = unit
            if unit_from:
                sd['unit_from'] = unit_from
            if divisor > 1 and kind != 'scaled':
                sd['scale'] = 1 / divisor
            sd['description'] = desc
            state[path] = sd
            writable = 'write' in o.get('access', '') or 'create' in o.get('access', '')
            if not writable or kind in ('oid', 'mac'):
                continue
            name = 'set_' + path.replace('.*', '').replace('.', '_')
            if name in commands:
                name = 'set_' + snake_words(words(o['name']))
            wire = None
            if kind == 'scaled':
                wire = 'TimeTicks' if o.get('base') == 'timeticks' else (
                    'Gauge32' if re.match(r'(Gauge|Unsigned32)', o.get('syntax', '')) else 'Integer')
            wire = wire or {'enum': 'Integer', 'ticks': 'TimeTicks', 'text': 'OctetString', 'ip': 'IpAddress',
                            'choice': 'ObjectIdentifier'}.get(kind)
            if kind == 'int':
                syn = o.get('syntax', '')
                wire = 'Gauge32' if re.match(r'(Gauge|Unsigned32)', syn) else 'Integer'
            params = {}
            entry_name = d[o['parent']]['name'] if idx else None
            for i, pn in zip(idx, index_params):
                io = d[i]
                lo, hi = io.get('range', (0, INT_MAX))
                params[pn] = {'type': 'int', 'min': lo, 'max': hi, 'required': True,
                              'label': ' '.join(w.capitalize() if j == 0 else w.lower() for j, w in enumerate(strip_words(i, prefix))),
                              'description': f"The row of {entry_name} to write, by its {i}."}
            vp = {'required': True, 'label': 'Value'}
            human = ' '.join(w.lower() for w in strip_words(o['name'], prefix))
            if kind in ('enum', 'choice'):
                vp = {'type': 'enum', 'values': [l for _, l in labels], **vp,
                      'description': f"The new {human}, as {cfg['mib']} names its values."}
            elif kind == 'scaled':
                lo, hi = o.get('range', (0, 4294967295) if wire in ('Gauge32', 'TimeTicks') else (INT_MIN, INT_MAX))
                vp = {'type': 'float', 'min': lo / divisor, 'max': hi / divisor, **vp,
                      'description': f"The new {human}, in {unit or 'units'}, in steps of {1 / divisor:g}."}
            elif kind in ('int', 'ticks'):
                lo, hi = o.get('range', (0, 4294967295) if wire in ('Gauge32', 'TimeTicks') else (INT_MIN, INT_MAX))
                step = f"1/{divisor} " if divisor > 1 else ''
                in_unit = f", in {step}{unit}." if unit else ", as the MIB counts it."
                vp = {'type': 'int', 'min': lo, 'max': hi, **vp, 'description': f"The new {human}{in_unit}"}
            elif kind == 'text':
                lo, hi = o.get('size', (0, 255))
                vp = {'type': 'string', 'max_length': hi, **vp, 'description': f"The new {human}."}
            elif kind == 'ip':
                vp = {'type': 'string', 'pattern': r'^(\d{1,3}\.){3}\d{1,3}$', 'max_length': 15, **vp,
                      'description': f"The new {human}, an IPv4 address."}
            params['value'] = vp
            row = f" of one {entry_name} row" if idx else ''
            commands[name] = {'summary': f"Set {o['name']}{row}, then read its {'table' if idx else 'group'} back",
                              'params': params, 'returns': 'ack'}
            lock = cfg.get('locks', {}).get(o['name'])
            setters.append({'command': name, 'oid': o['oid'], 'kind': kind, 'labels': labels, 'wire': wire,
                            'divisor': divisor,
                            'lock': d[lock]['oid'] if lock else '',
                            'indexes': index_params, 'reread': reread})
            for m in root['models']:
                supports[m].add(name)
    traps = []
    seen = set()
    for v in d.values():
        if v['kind'] in ('TRAP-TYPE', 'NOTIFICATION-TYPE') and 'oid' in v and v['oid'] not in seen:
            if any(v['oid'].startswith(p) for p in cfg.get('trap_prefixes', [])):
                seen.add(v['oid'])
                traps.append((v['oid'], v['name']))
    traps.sort(key=lambda t: [int(x) for x in t[0].split('.')])
    # A leaf must not also be a branch (a.b and a.b.c, or a.b and a.*.c): a
    # scalar beside a table of the same name joins its first two segments.
    for _ in range(3):
        segs = {p: p.split('.') for p in state}
        keys = list(state)
        rename = {}
        for i, p in enumerate(keys):
            for q in keys[i + 1:]:
                a, b = segs[p], segs[q]
                n = min(len(a), len(b))
                if len(a) != len(b) and all(x == y or x == '*' or y == '*' for x, y in zip(a[:n], b[:n])):
                    # the one without a '*' where the other has it
                    for x, other in ((p, q), (q, p)):
                        sx, so = segs[x], segs[other]
                        k = next((j for j in range(min(len(sx), len(so))) if so[j] == '*' and sx[j] != '*'), None)
                        if k is not None and k > 0:
                            rename[x] = '.'.join(sx[:k - 1] + [sx[k - 1] + '_' + sx[k]] + sx[k + 1:])
                            break
                    else:
                        print('state paths overlap:', p, q, file=sys.stderr)
        if not rename:
            break
        state = {rename.get(k, k): v for k, v in state.items()}
        for o in objects:
            o['path'] = rename.get(o['path'], o['path'])
    # A unit named by another object: that object's state path.
    by_name = {}
    for path, sd in state.items():
        m = re.match(r'(\w+) \(', sd['description'])
        if m:
            by_name[m.group(1)] = path
    for path, sd in state.items():
        if 'unit_from' in sd:
            source, suffix = sd['unit_from']
            sd['unit_from'] = {'state': by_name[source],
                               'units': {k: v + suffix for k, v in UNIT_FROM[source].items()}}
    return objects, setters, traps, state, commands, supports


def rust(cfg, objects, setters, traps):
    out = [f"//! The {cfg['title']} profile: generated from {cfg['mib']} {cfg['mib_version']} by",
           "//! tools/generate_snmp_profile.py. Do not edit by hand; run the tool again.",
           "",
           "use super::snmp_device::{Kind, Object, Profile, Root, Setter, Wire};",
           ""]
    enums = {}
    choices = {}
    for o in objects + setters:
        if o['kind'] == 'choice':
            key = tuple(o['labels'])
            if key not in choices:
                choices[key] = f"C{len(choices)}"
        if o['kind'] == 'enum':
            key = tuple(o['labels'])
            if key not in enums:
                enums[key] = f"E{len(enums)}"
    for key, name in enums.items():
        items = ', '.join(f'({v}, "{l}")' for v, l in key)
        out.append(f"static {name}: [(i64, &str); {len(key)}] = [{items}];")
    for key, name in choices.items():
        items = ', '.join('(&[' + ', '.join(v.split('.')) + f'], "{l}")' for v, l in key)
        out.append(f"static {name}: [(&[u32], &str); {len(key)}] = [{items}];")
    out.append('')

    def oid(s):
        return '&[' + ', '.join(s.split('.')) + ']'

    def kind(o):
        if o['kind'] == 'scaled':
            return f"Kind::Scaled({o['divisor']})"
        return {'int': 'Kind::Int', 'ticks': 'Kind::Ticks', 'text': 'Kind::Text', 'mac': 'Kind::Mac',
                'ip': 'Kind::Ip', 'oid': 'Kind::Oid'}.get(o['kind']) or (
            f"Kind::Choice(&{choices[tuple(o['labels'])]})" if o['kind'] == 'choice'
            else f"Kind::Enum(&{enums[tuple(o['labels'])]})")

    out.append(f"static OBJECTS: [Object; {len(objects)}] = [")
    for o in objects:
        out.append(f"    Object {{ oid: {oid(o['oid'])}, path: \"{o['path']}\", kind: {kind(o)} }},")
    out.append("];")
    out.append('')
    out.append(f"static SETTERS: [Setter; {len(setters)}] = [")
    for s in setters:
        idx = ', '.join(f'"{i}"' for i in s['indexes'])
        out.append(f"    Setter {{ command: \"{s['command']}\", oid: {oid(s['oid'])}, kind: {kind(s)}, wire: Wire::{s['wire']}, indexes: &[{idx}], reread: {oid(s['reread'])}, lock: {oid(s['lock']) if s['lock'] else '&[]'} }},")
    out.append("];")
    out.append('')
    if not cfg.get('traps_module'):
        out.append(f"static TRAPS: [(&[u32], &str); {len(traps)}] = [")
        for t, n in traps:
            out.append(f"    ({oid(t)}, \"{n}\"),")
        out.append("];")
        out.append('')
    out.append("pub(crate) static PROFILE: Profile = Profile {")
    out.append("    roots: &[")
    for r in cfg['roots']:
        ms = [] if set(r['models']) == set(cfg['models']) else r['models']
        models = ', '.join(f'"{m}"' for m in ms)
        out.append(f"        Root {{ oid: {oid(r['oid'])}, models: &[{models}] }},")
    out.append("    ],")
    out.append("    objects: &OBJECTS,")
    out.append("    setters: &SETTERS,")
    out.append("    traps: &" + (f"super::{cfg['traps_module']}::TRAPS" if cfg.get('traps_module') else "TRAPS") + ",")
    out.append("};")
    return '\n'.join(out) + '\n'


def yaml_value(v):
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return repr(v)
    if isinstance(v, dict):
        return '{ ' + ', '.join(f"{k}: {yaml_value(x)}" for k, x in v.items()) + ' }'
    if isinstance(v, list):
        return '[' + ', '.join(yaml_value(x) for x in v) + ']'
    s = str(v)
    return json.dumps(s, ensure_ascii=False) if (re.search(r'[:#{}\[\],&*?|<>=!%@`"\'\\]', s) or s in ('true', 'false', 'null', 'yes', 'no', 'on', 'off') or s == '' or re.match(r'^[\d.+-]', s)) else s


def yaml_commands(commands):
    out = []
    for name, c in commands.items():
        out.append(f"  {name}:")
        out.append(f"    summary: {yaml_value(c['summary'])}")
        out.append("    params:")
        for pn, p in c['params'].items():
            fields = ', '.join(f"{k}: {yaml_value(v)}" for k, v in p.items())
            out.append(f"      {pn}: {{ {fields} }}")
        out.append(f"    returns: {c['returns']}")
    return '\n'.join(out)


def yaml_state(state):
    out = []
    for path, s in state.items():
        fields = ', '.join(f"{k}: {yaml_value(v)}" for k, v in s.items())
        out.append(f"  {path}: {{ {fields} }}")
    return '\n'.join(out)


# ── Parts every SNMP spec shares ────────────────────────────────────────────

SETTINGS = r'''settings:
  version: { type: enum, values: [v1, v2c, v3], default: v2c, label: SNMP version,
             description: "The SNMP version the agent is asked with: v1 and v2c use a community string, v3 a user with authentication and privacy." }
  community: { type: string, secret: true, max_length: 255, default: public, label: Community,
               description: "v1 and v2c: the community string for reads, and for SETs when no write community is set." }
  write_community: { type: string, secret: true, max_length: 255, default: "", label: Write community,
                     description: "v1 and v2c: the community string SETs are sent with, often private; empty uses the community." }
  user: { type: string, max_length: 32, default: "", label: User,
          description: "v3: the user name (securityName) the agent knows." }
  security_level: { type: enum, values: [no_auth_no_priv, auth_no_priv, auth_priv], default: auth_priv, label: Security level,
                    description: "v3: no_auth_no_priv sends messages unauthenticated, auth_no_priv authenticates them, auth_priv also encrypts them." }
  auth_protocol: { type: enum, values: [md5, sha, sha224, sha256, sha384, sha512], default: sha, label: Authentication protocol,
                   description: "v3: HMAC-MD5-96, HMAC-SHA-96 (sha) or HMAC-SHA-2 (RFC 7860), as the user is set up on the agent." }
  auth_password: { type: string, secret: true, max_length: 256, default: "", label: Authentication password,
                   description: "v3: the user's authentication password, at least 8 characters." }
  priv_protocol: { type: enum, values: [des, aes, aes192, aes256], default: aes, label: Privacy protocol,
                   description: "v3 at auth_priv: CBC-DES, AES-128 (aes, RFC 3826), or AES-192 or AES-256 as draft-blumenthal-aes-usm-04 defines them." }
  priv_password: { type: string, secret: true, max_length: 256, default: "", label: Privacy password,
                   description: "v3 at auth_priv: the user's privacy password, at least 8 characters." }
  context_name: { type: string, max_length: 32, default: "", label: Context name,
                  description: "v3: the context requests are made in, empty for the agent's default context." }
  context_engine_id: { type: string, pattern: '^[0-9A-Fa-fxX :.-]*$', max_length: 100, default: "", label: Context engine ID,
                       description: "v3: the contextEngineID in hex, for an agent proxying another engine; empty uses the agent's own snmpEngineID." }
  timeout_ms: { type: int, min: 1, max: 60000, default: 1500, label: Timeout,
                description: "Milliseconds to wait for each response before sending the request again." }
  retries: { type: int, min: 0, max: 10, default: 2, label: Retries,
             description: "How many times an unanswered request is sent again before it fails with timeout." }
  max_repetitions: { type: int, min: 1, max: 1000, default: 20, label: GETBULK repetitions,
                     description: "Objects asked for per GETBULK request while walking." }
  poll_interval_ms: { type: int, min: 100, max: 86400000, default: 10000, label: Poll interval,
                      description: "Milliseconds between walks of the device's objects, and reads of sysUpTime.0, the liveness check." }
  trap_port: { type: int, min: 1, max: 65535, label: Notification port,
               description: "The UDP port traps and informs are received on, 162 as standard; unset receives none." }
  trap_bind_address: { type: string, pattern: '^[0-9A-Fa-f.:]*$', max_length: 45, default: "", label: Notification bind address,
                       description: "The local address to receive notifications on with a port of this device's own; empty shares the port with every SNMP device of the core, each hearing its own agent." }
  trap_community: { type: string, secret: true, max_length: 255, default: "", label: Notification community,
                    description: "v1 and v2c notifications: accept only this community; empty accepts any." }
  local_engine_id: { type: string, pattern: '^[0-9A-Fa-fxX :.-]*$', max_length: 100, default: "", label: Local engine ID,
                     description: "v3 informs: this receiver's snmpEngineID in hex, 5 to 32 bytes, as configured on the agent; empty uses a random one for each opening." }

ports:
  - { port: 161, protocol: udp, role: control, note: "the agent; requests go from an ephemeral port of the core's" }
  - { port: null, protocol: udp, role: notification, listener: core, setting: trap_port, note: "traps and informs, when trap_port is set (162 as standard)" }
'''

RAW_COMMANDS = r'''  get:
    summary: >
      GET objects by OID and return {varbinds: [{oid, type, value, hex}]}, for objects the named commands and state
      do not cover; an object state holds is updated from the answer too
    params:
      oids: { type: string, pattern: '^[0-9., ;]+$', max_length: 65536, required: true, label: OIDs,
              description: "One or more OIDs in dotted decimal, separated by commas or spaces, such as 1.3.6.1.2.1.1.3.0." }
    returns: value
  walk:
    summary: Every object under an OID, in order, as {varbinds}
    params:
      oid: { type: string, pattern: '^\.?[0-9]+(\.[0-9]+)+$', max_length: 1024, required: true, label: OID,
             description: "The subtree's root in dotted decimal." }
      limit: { type: int, min: 1, max: 100000000, label: Object limit,
               description: "Stop after this many objects; unset walks the whole subtree." }
    returns: value
  set:
    summary: >
      SET one object by OID to a typed value and return the agent's response as {varbinds}; fails with the agent's
      error-status (notWritable, wrongValue...) as the code
    params:
      oid: { type: string, pattern: '^\.?[0-9]+(\.[0-9]+)+$', max_length: 1024, required: true, label: OID,
             description: "The object instance to set, in dotted decimal." }
      type: { type: enum, values: [integer, string, hex, oid, ip_address, counter32, gauge32, timeticks, counter64, opaque], required: true, label: Type,
              description: "The value's SNMP type: string is UTF-8 text and hex is bytes for an OCTET STRING; gauge32 is also Unsigned32." }
      value: { type: string, max_length: 65536, required: true, label: Value,
               description: "The value as text: a decimal number, text, hex bytes, a dotted OID or an IPv4 address, as the type asks." }
    returns: value
  refresh:
    summary: Read the system group and walk every object now; answered when they have been read
    returns: ack
'''

COMMON_STATE = r'''  polled.status: { type: int, unit: s, description: "When the status poll (a walk of the device's tables at its interval) was last answered, in Unix seconds: the polled state is as old as this" }
  version: { type: string, description: "The SNMP version used: v1, v2c or v3" }
  system.description: { type: string, description: "sysDescr.0: the agent's description of the device" }
  system.object_id: { type: string, description: "sysObjectID.0: the vendor's OID for the device type" }
  system.uptime: { type: float, unit: s, description: "sysUpTime.0, its TimeTicks in seconds" }
  system.contact: { type: string, description: "sysContact.0" }
  system.name: { type: string, description: "sysName.0" }
  system.location: { type: string, description: "sysLocation.0" }
  engine.id: { type: string, description: "v3: the agent's snmpEngineID in hex" }
  engine.boots: { type: int, description: "v3: the agent's snmpEngineBoots" }
  engine.time: { type: int, unit: s, description: "v3: the agent's snmpEngineTime when last heard" }
  local_engine_id: { type: string, description: "With trap_port: this receiver's snmpEngineID in hex, to configure on agents sending v3 informs" }
  traps.count: { type: int, description: "Notifications received since opening" }
  traps.last: { type: object, description: "The last notification: {kind (trap or inform), version, source, trap_oid, name (the MIB's name for it, or null), uptime (the agent's uptime when it sent it, in seconds), varbinds, and for v1 enterprise, agent_address, generic_trap, specific_trap, and for v3 user}" }
'''


COMMON_POLLS = r'''  - name: status
    every_ms: { setting: poll_interval_ms }
    reads: "sysUpTime.0, the liveness check, and a walk of the device's subtrees for its model"
    note: "Opened for commands only, only sysUpTime.0 is read. A notification received brings the next walk forward to 500 ms after it; a walk still unanswered is not started again."
'''


# ── The spec ────────────────────────────────────────────────────────────────

def assemble(spec_text, cfg, state, commands, supports):
    """The spec with its settings, commands and state replaced, and each
    model's supports; its head and quirks as they were."""
    head, rest = spec_text.split('\nsettings:\n', 1)
    quirks = rest[rest.index('\nquirks:\n') + 1:] if '\nquirks:\n' in rest else ''
    raw = ['get', 'walk', 'set', 'refresh']
    for m, sup in supports.items():
        pattern = r'(\n  - id: ' + re.escape(m) + r'\n(?:    .*\n)*?    supports: )\[[^\]]*\]'
        head, n = re.subn(pattern, lambda x: x.group(1) + '[' + ', '.join(raw + sorted(sup)) + ']', head)
        assert n == 1, f"model {m} has no supports line"
    nl = '\n'
    return (head.rstrip() + nl + nl + SETTINGS + nl + 'commands:' + nl + RAW_COMMANDS + yaml_commands(commands) +
            nl + nl + 'polls:' + nl + COMMON_POLLS + nl + 'state:' + nl + COMMON_STATE + yaml_state(state) + nl +
            ((nl + quirks.strip() + nl) if quirks else ''))


def traps_rust(cfg, traps):
    out = [f"//! The notifications of {cfg['mib']} {cfg['mib_version']} by name: generated by",
           "//! tools/generate_snmp_profile.py. Do not edit by hand; run the tool again.", "",
           f"pub(crate) static TRAPS: [(&[u32], &str); {len(traps)}] = ["]
    for t, n in traps:
        out.append("    (&[" + ', '.join(t.split('.')) + f"], \"{n}\"),")
    out.append("];")
    return '\n'.join(out) + '\n'


def write_rust(path, text):
    path.write_text(text, encoding='utf8', newline='\n')
    subprocess.run(['rustfmt', '--edition', '2021', str(path)], check=True)


def main(argv):
    if len(argv) != 2 or argv[0] not in PROFILES:
        sys.exit(__doc__)
    sid, mib_file = argv
    cfg = dict(PROFILES[sid])
    text = Path(mib_file).read_text(encoding='latin-1')
    # An RFC's page breaks interrupt its MIB.
    text = re.sub(r'\n?[^\n]*\[Page \d+\]\s*\n\x0c?\s*RFC \d+[^\n]*\n', '\n', text)
    defs = parse_text(text)
    cfg['roots'] = [dict(r, oid=defs[r['name']]['oid']) for r in cfg['roots']]
    objects, setters, traps, state, commands, supports = generate(defs, cfg)
    spec = ROOT / 'specs' / f'{sid}.yaml'
    spec.write_text(assemble(spec.read_text(encoding='utf8'), cfg, state, commands, supports),
                    encoding='utf8', newline='\n')
    modules = ROOT / 'crates' / 'core' / 'src' / 'modules'
    write_rust(modules / (sid.replace('-', '_') + '_profile.rs'), rust(cfg, objects, setters, traps))
    if cfg.get('traps_module'):
        write_rust(modules / (cfg['traps_module'] + '.rs'), traps_rust(cfg, traps))
    print(f"{sid}: {len(objects)} objects, {len(setters)} commands, {len(traps)} notifications")


if __name__ == '__main__':
    main(sys.argv[1:])
