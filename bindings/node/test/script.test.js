'use strict';

// Plays tests/bindings/script.json through the raw Node binding against
// integrations-sim. Every delivery runs the same script; any difference in
// what a binding passes through fails here.

const test = require('node:test');
const assert = require('node:assert');
const { spawn, execFileSync } = require('node:child_process');
const { join } = require('node:path');
const readline = require('node:readline');

const root = join(__dirname, '..', '..', '..');
const { NativeCore } = require(join(__dirname, '..', `meros-integrations.${process.platform}-${process.arch}.node`));

function startSimulator() {
  execFileSync('cargo', ['build', '-q', '-p', 'meros-integrations-sim'], { cwd: root, stdio: 'inherit' });
  const exe = join(root, 'target', 'debug', process.platform === 'win32' ? 'integrations-sim.exe' : 'integrations-sim');
  const child = spawn(exe, [], { stdio: ['pipe', 'pipe', 'inherit'] });
  return new Promise((resolve) => {
    readline.createInterface({ input: child.stdout }).once('line', (line) => resolve({ child, ports: JSON.parse(line) }));
  });
}

function stripMessages(value) {
  if (Array.isArray(value)) return value.map(stripMessages);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).filter(([k]) => k !== 'message').map(([k, v]) => [k, stripMessages(v)]));
  }
  return value;
}

function substitute(value, vars) {
  if (typeof value === 'string' && value.startsWith('$')) return vars[value.slice(1)];
  if (Array.isArray(value)) return value.map((v) => substitute(v, vars));
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, substitute(v, vars)]));
  }
  return value;
}

function at(value, path) {
  return path.split('.').reduce((node, key) => (node == null ? undefined : node[key]), value);
}

test('the shared binding script', async () => {
  const script = require(join(root, 'tests', 'bindings', 'script.json'));
  const { child, ports } = await startSimulator();
  const core = new NativeCore();
  const vars = { ...ports };
  try {
    for (const [index, raw] of script.steps.entries()) {
      const step = substitute(raw, vars);
      const label = `step ${index}: ${JSON.stringify(raw)}`;
      switch (step.op) {
        case 'catalog':
          assert.deepStrictEqual(at(core.catalog(), step.path), step.expect, label);
          break;
        case 'open': {
          const result = core.open(step.request);
          if (step.expect) assert.deepStrictEqual(stripMessages(result), step.expect, label);
          if (step.save) {
            assert.ok(result.device !== undefined, `${label}: ${JSON.stringify(result)}`);
            vars[step.save] = result.device;
          }
          break;
        }
        case 'wait': {
          const deadline = Date.now() + 5000;
          for (;;) {
            if (at(core.snapshot(step.device), step.path) === step.equals) break;
            assert.ok(Date.now() < deadline, `${label}: timed out`);
            await new Promise((r) => setTimeout(r, 20));
          }
          break;
        }
        case 'execute': {
          const result = await core.execute(step.device, step.command, step.params);
          assert.deepStrictEqual(stripMessages(result), step.expect, label);
          break;
        }
        case 'close':
          await core.close(step.device);
          break;
        default:
          throw new Error(`unknown op ${step.op}`);
      }
    }
  } finally {
    core.interruptEvents();
    child.stdin.end();
  }
});
