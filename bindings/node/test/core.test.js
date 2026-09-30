'use strict';

// Drives a simulated Digital 6000 through the Node binding, proving the
// binding passes the core's behaviour through unchanged: connection events,
// state, acknowledged commands, and errors as rejected promises.

const test = require('node:test');
const assert = require('node:assert');
const dgram = require('node:dgram');

const { Core, IntegrationsError } = require('..');

function simulatedEm6000() {
  const socket = dgram.createSocket('udp4');
  socket.on('message', (buf, from) => {
    const msg = JSON.parse(buf.toString());
    const reply = (m) => socket.send(JSON.stringify(m), from.port, from.address);
    if (msg.osc?.state?.subscribe) {
      reply({ rx1: { name: 'Lead', carrier: 606000, audio_mute: false, active_warnings: [] } });
    } else if (msg.rx1 && 'audio_mute' in msg.rx1) {
      reply({ rx1: { audio_mute: msg.rx1.audio_mute } });
    }
  });
  return new Promise((resolve) => socket.bind(0, '127.0.0.1', () => resolve(socket)));
}

// Records every event from the moment it is attached, so a wait can match an
// event that arrived before the wait began. Events come in batches: waiting
// for "connected" and only then listening for the first state patch misses a
// patch delivered in the same batch.
function recorder(core) {
  const seen = [];
  core.on('event', (event) => seen.push(event));
  return async function until(predicate) {
    const deadline = Date.now() + 5000;
    for (;;) {
      const found = seen.find(predicate);
      if (found) return found;
      if (Date.now() > deadline) throw new Error('timed out');
      await new Promise((r) => setTimeout(r, 10));
    }
  };
}

test('drives a device through the binding', async () => {
  const device = await simulatedEm6000();
  const core = new Core();
  const until = recorder(core);
  try {
    const id = core.open({
      device: 'sennheiser-digital-6000',
      model: 'em-6000',
      host: '127.0.0.1',
      port: device.address().port,
    });

    await until((e) => e.event === 'connection' && e.device === id && e.connection.status === 'connected');
    await until((e) => e.event === 'state' && e.patch.channels?.['1']?.name === 'Lead');
    assert.strictEqual(core.snapshot(id).state.channels['1'].frequency_khz, 606000);

    assert.deepStrictEqual(await core.execute(id, 'mute', { channel: 1, muted: true }), { kind: 'ack' });

    await assert.rejects(core.execute(id, 'mute', { channel: 9 }), (err) => {
      assert.ok(err instanceof IntegrationsError);
      assert.strictEqual(err.code, 'invalid_params');
      return true;
    });
    assert.throws(() => core.open({ device: 'nope', model: 'x', host: '127.0.0.1' }), /unknown device/);

    await core.close(id);
  } finally {
    core.dispose();
    device.close();
  }
});

test('the catalogue lists every spec', () => {
  const core = new Core();
  try {
    const { devices } = core.catalog();
    assert.ok(devices['sennheiser-ew-dx']);
    assert.ok(devices['shure-wireless']);
  } finally {
    core.dispose();
  }
});
