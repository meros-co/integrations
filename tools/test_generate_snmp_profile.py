"""Tests of tools/generate_snmp_profile.py's units: python -m unittest discover -s tools -p "test_*.py"."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_snmp_profile as g  # noqa: E402


class Units(unittest.TestCase):
    def test_a_unit_and_its_step(self):
        self.assertEqual(g.unit_of({'name': 'x', 'description': 'The load in tenths of Amps.'}), ('A', 10))
        self.assertEqual(g.unit_of({'name': 'x', 'units': '0.1 Hertz'}), ('Hz', 10))
        self.assertEqual(g.unit_of({'name': 'x', 'units': 'RMS Volts'}), ('V', 1))
        self.assertEqual(g.unit_of({'name': 'x', 'base': 'timeticks'}), ('s', 100))
        self.assertEqual(g.unit_of({'name': 'x', 'description': 'A count.'}), (None, 1))

    def test_a_profiles_table_names_what_the_wording_does_not(self):
        cfg = g.PROFILES['cyberpower-ups']
        self.assertTrue(cfg['si'])
        line = {'name': 'upsAdvanceInputLineVoltage', 'description': 'The input line voltage in 1/10 VAC.'}
        self.assertEqual(g.unit_of(line, cfg), ('V', 10))
        self.assertEqual(g.unit_of(line), (None, 1))
        self.assertEqual(g.unit_of({'name': 'envir2Temperature'}, cfg), ('@envir2TempUnit', 100, ''))

    def test_a_step_is_written_as_a_number(self):
        self.assertEqual(g.yaml_value(0.1), '0.1')
        self.assertEqual(g.yaml_value({'state': 'a.b', 'units': {'c': '°C'}}), '{ state: a.b, units: { c: °C } }')


if __name__ == '__main__':
    unittest.main()
