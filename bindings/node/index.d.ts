import { EventEmitter } from 'node:events';

export type DeviceId = number;

export interface OpenRequest {
  /** Spec id, e.g. 'sennheiser-ew-dx'. */
  device: string;
  /** Model id within the spec, e.g. 'em-2'. */
  model: string;
  /**
   * IP address or hostname. May be left out for an integration on which only
   * the core listens (every port `listener: core`), such as 'osc-listener'.
   */
  host?: string;
  /** The device's port, when it is not the protocol's default. */
  port?: number;
  /** Settings declared by the spec, such as a password. */
  settings?: Record<string, unknown>;
  /**
   * Keep the device's state current (the default). `false` opens it for
   * commands only: no subscription, connect-time read or poll, so none of the
   * device's limited subscription slots is taken. What the device sends
   * unasked is still applied to the state.
   */
  monitor?: boolean;
}

export interface DiscoverRequest {
  /** Listen passively, scan now (and listen), or stop. */
  action: 'listen' | 'scan' | 'stop';
  /**
   * Discovery protocols: 'mcp' (Sennheiser G3/G4), 'ssdp' (Sony cameras),
   * 'pjlink' (PJLink projectors), 'mdns' (Blackmagic ATEM, Videohub,
   * HyperDeck, SmartView and MultiView). Empty means every protocol that
   * finds a device in this core's catalogue; naming one that finds none is
   * an error.
   */
  protocols?: string[];
  /**
   * Addresses where devices were last seen. An MCP scan also sweeps their
   * /24s; SSDP, PJLink and mDNS scans also ask each by unicast.
   */
  hints?: string[];
}

export type Outcome =
  | { kind: 'ack' }
  | { kind: 'value'; value: unknown }
  /** Sent, but the protocol gives no way to confirm it was applied. */
  | { kind: 'unverified' };

export type Connection =
  | { status: 'connecting' }
  | { status: 'connected' }
  | { status: 'disconnected'; reason: string }
  /** Reachable, but the device refused the configured credentials. */
  | { status: 'unauthorized'; reason: string }
  /** The protocol never replies, so presence cannot be known. */
  | { status: 'unmonitored' };

export interface Snapshot {
  connection: Connection;
  /** Last known state; `connection` says whether it is current. */
  state: Record<string, unknown>;
  /** The most recent request-to-reply time, where the protocol answers requests. */
  latency_ms?: number;
}

export type Event =
  | { event: 'connection'; device: DeviceId; connection: Connection }
  /** An RFC 7386 merge patch against the device's state. */
  | { event: 'state'; device: DeviceId; patch: Record<string, unknown> }
  /**
   * Heard from the device. At most once per device per second. `latency_ms`
   * is the most recent request-to-reply time, where the protocol answers.
   */
  | { event: 'alive'; device: DeviceId; latency_ms?: number }
  | { event: 'log'; device: DeviceId; level: 'debug' | 'info' | 'warning'; message: string }
  | { event: 'closed'; device: DeviceId }
  /**
   * New values of the device's settings the core obtained itself: OAuth
   * tokens it refreshed for an `auth: oauth2` device. Secrets, held by the
   * core in memory only: persist them and pass them when the device is next
   * opened. `refresh_token` is present only when the service rotated it, and
   * then replaces the stored one. Never discarded.
   */
  | { event: 'credentials'; device: DeviceId; settings: CredentialSettings }
  /**
   * One message a listener received (an OSC control surface's button), every
   * time, even when it repeats the last: a button pressed twice is two events.
   * `source` is the sender's ip:port; `types` the OSC type tags without the
   * comma. Kept when the consumer falls behind, unlike state patches.
   */
  | {
      event: 'message';
      device: DeviceId;
      address: string;
      types?: string;
      args: unknown[];
      source: string;
    }
  /**
   * `count` state patches were discarded; resynchronise from snapshots.
   * `messages` message events were discarded past the queue's hard ceiling
   * (absent when none were).
   */
  | { event: 'dropped'; count: number; messages?: number }
  /**
   * A device found by discovery, or more learned about one. `models` lists
   * every model it can be; `evidence` says what the identification rests on.
   */
  | {
      event: 'discovered';
      protocol: string;
      address: string;
      port: number;
      device: string;
      models: string[];
      name?: string;
      evidence: Record<string, string>;
    }
  | { event: 'discovery'; protocol: string; message: string };

export interface CredentialSettings {
  access_token: string;
  /** When the access token expires, in Unix seconds; null when the service gave no lifetime. */
  expires_at: number | null;
  /** Present only when the service issued a new refresh token. */
  refresh_token?: string;
  [setting: string]: unknown;
}

export type ErrorCode =
  | 'invalid_request'
  | 'invalid_params'
  | 'unknown_command'
  | 'unsupported_for_model'
  | 'not_connected'
  | 'timeout'
  | 'device_error'
  | 'transport'
  | 'auth'
  | 'closed'
  | 'unknown_device'
  | 'not_selected'
  | 'not_built'
  | 'unknown_model'
  | 'invalid_settings'
  | 'unresolvable_host'
  | 'not_implemented'
  | 'unknown_stream'
  | 'closing';

export class IntegrationsError extends Error {
  readonly code: ErrorCode;
  readonly detail: { error: ErrorCode; message: string; [key: string]: unknown };
}

export interface CoreOptions {
  /** The local network interface address for device traffic; every interface when absent. */
  bindAddress?: string;
  /**
   * The only integrations this core works with: spec ids, vendor groups
   * ('vendor-sennheiser') or 'all'; every integration in the build when
   * absent. The catalogue then lists only them, opening any other throws
   * 'not_selected', and discovery runs only their protocols. A name the
   * build does not include makes the constructor throw.
   */
  devices?: string[];
}

export interface Frame {
  /** The encoding the spec declares, e.g. 'jpeg'. */
  format: string;
  data: Buffer;
  /** Rises by one per frame the device published; a gap is frames not seen. */
  sequence: number;
  /** Frames replaced before they were taken, since the last frame. */
  dropped: number;
}

/**
 * One watcher of a device's stream. Only the newest frame is kept for a slow
 * listener. The device produces frames while any stream on it is open.
 */
export class Stream extends EventEmitter {
  /** Stop watching; 'end' follows. */
  close(): void;
  on(event: 'frame', listener: (frame: Frame) => void): this;
  /** Closed, or the device's session ended. */
  on(event: 'end', listener: () => void): this;
}

export class Core extends EventEmitter {
  constructor(options?: CoreOptions);
  catalog(): { devices: Record<string, unknown> };
  open(request: OpenRequest): DeviceId;
  /** Found devices arrive as `discovered` events. */
  discover(request: DiscoverRequest): void;
  execute(device: DeviceId, command: string, params?: Record<string, unknown>): Promise<Outcome>;
  /**
   * Change an open device's settings without closing it: merged into its
   * current ones (null puts one back to its default) and validated against
   * the spec first. A spec-driven device takes them live (a refused
   * credential is let go and it connects again); a native one is restarted
   * under the same id. Rejects with an IntegrationsError ('invalid_settings',
   * 'closed'), having changed nothing.
   */
  updateSettings(device: DeviceId, settings: Record<string, unknown>): Promise<void>;
  snapshot(device: DeviceId): Snapshot | null;
  close(device: DeviceId): Promise<void>;
  /**
   * Shut down cleanly before the process exits: refuse new devices and
   * commands ('closing'), give commands in flight up to `graceMs` (default
   * 5000), then close every device so each ends what it started on it.
   */
  closeAll(graceMs?: number): Promise<void>;
  /**
   * Watch a stream declared under the device's `streams` in the catalogue,
   * such as a camera's 'live' preview. Throws an IntegrationsError
   * ('unknown_stream', 'unsupported_for_model', 'closed').
   */
  openStream(device: DeviceId, stream: string): Stream;
  /** Stop delivering events so the process can exit. */
  dispose(): void;

  on(event: 'event', listener: (event: Event) => void): this;
  on(event: 'connection', listener: (event: Extract<Event, { event: 'connection' }>) => void): this;
  on(event: 'state', listener: (event: Extract<Event, { event: 'state' }>) => void): this;
  on(event: 'alive', listener: (event: Extract<Event, { event: 'alive' }>) => void): this;
  on(event: 'log', listener: (event: Extract<Event, { event: 'log' }>) => void): this;
  on(event: 'closed', listener: (event: Extract<Event, { event: 'closed' }>) => void): this;
  on(event: 'message', listener: (event: Extract<Event, { event: 'message' }>) => void): this;
  on(event: 'credentials', listener: (event: Extract<Event, { event: 'credentials' }>) => void): this;
  on(event: 'dropped', listener: (event: Extract<Event, { event: 'dropped' }>) => void): this;
  on(event: 'discovered', listener: (event: Extract<Event, { event: 'discovered' }>) => void): this;
  on(event: 'discovery', listener: (event: Extract<Event, { event: 'discovery' }>) => void): this;
}
