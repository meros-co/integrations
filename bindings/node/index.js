'use strict';

// The idiomatic JavaScript surface over the native core.
//
// The native layer returns JSON shaped by the core itself; this file only turns
// `{error}` results into rejected promises and the event queue into an
// EventEmitter. It holds no protocol logic, retries or timeouts.

const { EventEmitter } = require('node:events');
const { join } = require('node:path');

const { NativeCore } = require(join(__dirname, `meros-integrations.${process.platform}-${process.arch}.node`));

/** A command, open or validation failure, carrying the core's error object. */
class IntegrationsError extends Error {
  constructor(detail) {
    super(detail.message);
    this.name = 'IntegrationsError';
    /** e.g. 'device_error', 'invalid_params', 'not_connected' */
    this.code = detail.error;
    this.detail = detail;
  }
}

/**
 * One watcher of a device's stream, such as a camera's 'live' preview.
 * Emits 'frame' with { format, data: Buffer, sequence, dropped } for each
 * frame, then 'end' once, when closed or when the device's session ends.
 * Only the newest frame is kept for a slow listener; `dropped` counts the
 * frames it replaced. The device produces frames while a stream is open.
 */
class Stream extends EventEmitter {
  #native;
  #id;
  #closed = false;

  constructor(native, id) {
    super();
    this.#native = native;
    this.#id = id;
    // Start delivering once the caller has had a chance to add listeners.
    setImmediate(() => this.#pump());
  }

  /** Stop watching. Emits 'end'. */
  close() {
    if (this.#closed) return;
    this.#closed = true;
    this.#native.closeStream(this.#id);
  }

  async #pump() {
    try {
      for (;;) {
        const frame = await this.#native.nextFrame(this.#id);
        if (frame === null || frame === undefined) break;
        this.emit('frame', frame);
      }
    } finally {
      this.close();
      this.emit('end');
    }
  }
}

/**
 * Emits 'event' for every core event, and also the event under its own name:
 * 'connection', 'state', 'alive', 'log', 'closed', 'dropped'.
 */
class Core extends EventEmitter {
  #native;
  #pumping = false;
  #disposed = false;

  /**
   * @param {{ bindAddress?: string, devices?: string[] }} [options]
   *   bindAddress: the local network interface for device traffic; every
   *   interface when absent. devices: the only integrations this core works
   *   with: spec ids, vendor groups ('vendor-sennheiser') or 'all'; every
   *   integration in the build when absent. Throws for a name the build does
   *   not include.
   */
  constructor(options = {}) {
    super();
    const native = {};
    if (options.bindAddress) native.bind_address = options.bindAddress;
    if (options.devices) native.devices = options.devices;
    this.#native = new NativeCore(Object.keys(native).length ? native : null);
  }

  catalog() {
    return this.#native.catalog();
  }

  /** Open a device; returns its id. The connection is made in the background. */
  open(request) {
    const result = this.#native.open(request);
    if (result.error) throw new IntegrationsError(result.error);
    this.#pump();
    return result.device;
  }

  /**
   * Listen for devices, scan for them now, or stop. Found devices arrive as
   * 'discovered' events. Throws an IntegrationsError for an invalid request.
   */
  discover(request) {
    const result = this.#native.discover(request);
    if (result.error) throw new IntegrationsError(result.error);
    this.#pump();
  }

  /** Resolves with the outcome, or rejects with an IntegrationsError. */
  async execute(device, command, params = {}) {
    const result = await this.#native.execute(device, command, params);
    if (result.error) throw new IntegrationsError(result.error);
    return result.ok;
  }

  /** Last known state and connection status, or null if not open. */
  snapshot(device) {
    return this.#native.snapshot(device);
  }

  async close(device) {
    await this.#native.close(device);
  }

  /**
   * Shut down cleanly before the process exits: refuse new devices and
   * commands, give commands in flight up to `graceMs`, then close every
   * device so each ends what it started on the device.
   */
  async closeAll(graceMs = 5000) {
    await this.#native.closeAll(graceMs);
  }

  /**
   * Watch a stream the catalogue declares under the device's `streams`.
   * Returns a Stream emitting 'frame' and 'end'. Throws an IntegrationsError
   * ('unknown_stream', 'unsupported_for_model', 'closed').
   */
  openStream(device, stream) {
    const result = this.#native.openStream(device, stream);
    if (result.error) throw new IntegrationsError(result.error);
    return new Stream(this.#native, result.stream);
  }

  /** Stop delivering events so the process can exit. */
  dispose() {
    this.#disposed = true;
    this.#native.interruptEvents();
  }

  async #pump() {
    if (this.#pumping || this.#disposed) return;
    this.#pumping = true;
    try {
      while (!this.#disposed) {
        const events = await this.#native.nextEvents(256);
        for (const event of events) {
          this.emit('event', event);
          this.emit(event.event, event);
        }
      }
    } finally {
      this.#pumping = false;
    }
  }
}

module.exports = { Core, IntegrationsError, Stream };
