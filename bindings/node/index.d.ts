import { EventEmitter } from 'node:events';

export type DeviceId = number;

export interface OpenRequest {
  /** Spec id, e.g. 'sennheiser-ew-dx'. */
  device: string;
  /** Model id within the spec, e.g. 'em-2'. */
  model: string;
  /** IP address or hostname. */
  host: string;
  /** Settings declared by the spec, such as a password. */
  settings?: Record<string, unknown>;
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
  | { status: 'unauthorized'; reason: string };

export interface Snapshot {
  connection: Connection;
  /** Last known state; `connection` says whether it is current. */
  state: Record<string, unknown>;
}

export type Event =
  | { event: 'connection'; device: DeviceId; connection: Connection }
  /** An RFC 7386 merge patch against the device's state. */
  | { event: 'state'; device: DeviceId; patch: Record<string, unknown> }
  /** Heard from the device. At most once per device per second. */
  | { event: 'alive'; device: DeviceId }
  | { event: 'log'; device: DeviceId; level: 'debug' | 'info' | 'warning'; message: string }
  | { event: 'closed'; device: DeviceId }
  /** State patches were discarded; resynchronise from snapshots. */
  | { event: 'dropped'; count: number };

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
  | 'unknown_model'
  | 'invalid_settings'
  | 'unresolvable_host'
  | 'not_implemented';

export class IntegrationsError extends Error {
  readonly code: ErrorCode;
  readonly detail: { error: ErrorCode; message: string; [key: string]: unknown };
}

export class Core extends EventEmitter {
  catalog(): { devices: Record<string, unknown> };
  open(request: OpenRequest): DeviceId;
  execute(device: DeviceId, command: string, params?: Record<string, unknown>): Promise<Outcome>;
  snapshot(device: DeviceId): Snapshot | null;
  close(device: DeviceId): Promise<void>;
  /** Stop delivering events so the process can exit. */
  dispose(): void;

  on(event: 'event', listener: (event: Event) => void): this;
  on(event: 'connection', listener: (event: Extract<Event, { event: 'connection' }>) => void): this;
  on(event: 'state', listener: (event: Extract<Event, { event: 'state' }>) => void): this;
  on(event: 'alive', listener: (event: Extract<Event, { event: 'alive' }>) => void): this;
  on(event: 'log', listener: (event: Extract<Event, { event: 'log' }>) => void): this;
  on(event: 'closed', listener: (event: Extract<Event, { event: 'closed' }>) => void): this;
  on(event: 'dropped', listener: (event: Extract<Event, { event: 'dropped' }>) => void): this;
}
