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
 * Emits 'event' for every core event, and also the event under its own name:
 * 'connection', 'state', 'alive', 'log', 'closed', 'dropped'.
 */
class Core extends EventEmitter {
  #native;
  #pumping = false;
  #disposed = false;

  /**
   * @param {{ bindAddress?: string }} [options] bindAddress: the local network
   *   interface for device traffic; every interface when absent.
   */
  constructor(options = {}) {
    super();
    this.#native = new NativeCore(options.bindAddress ? { bind_address: options.bindAddress } : null);
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

module.exports = { Core, IntegrationsError };
