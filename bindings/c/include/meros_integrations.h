/*
 * C interface to the integrations core.
 *
 * JSON strings in, JSON strings out, in the same shapes every delivery of the
 * core returns. Every string a function returns is owned by the caller and is
 * freed with mi_string_free. Calls block: make them from a worker thread, and
 * never from inside a callback of this library (there are none). No call
 * unwinds into C; an internal fault is returned as {"error":{"error":"internal"}}.
 */
#ifndef MEROS_INTEGRATIONS_H
#define MEROS_INTEGRATIONS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MiCore MiCore;

/* Start a core, with its own threads. NULL if it cannot start. */
MiCore *mi_core_new(void);
/* options: {"bind_address":"<local interface address>",
 *           "devices":["<spec id>",...]}, or NULL for defaults.
 * devices: the only integrations this core works with (spec ids, vendor
 * groups such as "vendor-sennheiser", or "all"); absent means every
 * integration in the build. The catalogue then lists only them, mi_open of any other
 * fails with "not_selected", and discovery runs only their protocols.
 * NULL if the options are invalid (including an unknown device id) or the
 * core cannot start. */
MiCore *mi_core_new_with_options(const char *options);
/* As mi_core_new_with_options, and on failure, if error is not NULL, sets
 * *error to {"error":{"error":"invalid_options"|"internal","message":"..."}}
 * (free it with mi_string_free). */
MiCore *mi_core_create(const char *options, char **error);
void mi_core_free(MiCore *core);
void mi_string_free(char *s);

/* Every device spec: models, commands, parameters, quirks. */
char *mi_catalog(const MiCore *core);

/* request: {"device":"<spec id>","model":"<model id>","host":"<ip or name>",
 *           "port":<optional>,"settings":{...},"monitor":<optional bool>}
 * "monitor":false opens the device for commands only: no subscription,
 * connect-time read or poll.
 * returns: {"device":<id>} or {"error":{...}} */
char *mi_open(const MiCore *core, const char *request);

/* request: {"action":"listen"|"scan"|"stop","protocols":["mcp"],
 *           "hints":["<address where a device was last seen>",...]}
 * returns: {"ok":true} or {"error":{...}}; found devices arrive as
 * {"event":"discovered",...} events. */
char *mi_discover(const MiCore *core, const char *request);

/* params: a JSON object, or NULL for none.
 * returns: {"ok":{"kind":"ack"|"value"|"unverified",...}} or {"error":{...}} */
char *mi_execute(const MiCore *core, uint64_t device, const char *command, const char *params);

/* {"connection":{...},"state":{...}}, or null if the device is not open. */
char *mi_snapshot(const MiCore *core, uint64_t device);

/* A JSON array of events. mi_poll_events never waits; mi_wait_events waits up
 * to timeout_ms and returns [] on timeout or after mi_interrupt_events. */
char *mi_poll_events(const MiCore *core, uint32_t max);
char *mi_wait_events(const MiCore *core, uint32_t max, uint32_t timeout_ms);
void mi_interrupt_events(const MiCore *core);

/* End a device's session and wait until it has closed. */
void mi_close(const MiCore *core, uint64_t device);

/* Shut down cleanly before exiting: refuse new devices and commands
 * (open answers {"error":{"error":"closing"}}), give commands in flight up
 * to grace_ms, then close every device so each ends what it started on the
 * device (subscriptions, metering). Call before mi_core_free. */
void mi_close_all(const MiCore *core, uint32_t grace_ms);

/* Streams: continuous media such as a camera's live view, declared in the
 * catalogue under a device's "streams". Frames do not come as events: each
 * open stream holds only the newest frame, and frames replaced before they
 * were taken are counted in `dropped`. The device produces frames only while
 * at least one stream is open on it.
 *
 * Each frame is owned by the caller and freed with mi_frame_free; `data`
 * stays valid until then. Free every stream with mi_stream_free before
 * mi_core_free. */
typedef struct MiStream MiStream;

typedef struct MiFrame {
    const uint8_t *data;   /* the encoded frame, len bytes */
    size_t len;
    const char *format;    /* "jpeg" */
    uint64_t sequence;     /* rises by one per frame the device published */
    uint64_t dropped;      /* frames replaced unseen since the last one taken */
} MiFrame;

/* Watch a stream. returns: {"ok":true} with *out set, or {"error":{...}}
 * ("unknown_stream", "unsupported_for_model", "closed") with *out NULL. */
char *mi_stream_open(const MiCore *core, uint64_t device, const char *stream, MiStream **out);
/* The newest frame without waiting, or NULL. */
MiFrame *mi_stream_poll(const MiStream *stream);
/* Wait up to timeout_ms for a frame. NULL on timeout, or once the stream is
 * closed or the device's session has ended (mi_stream_ended returns 1). */
MiFrame *mi_stream_wait(const MiCore *core, const MiStream *stream, uint32_t timeout_ms);
int mi_stream_ended(const MiStream *stream);
/* Stop watching; a waiting mi_stream_wait returns NULL. Callable from any
 * thread. The stream must still be freed. */
void mi_stream_close(const MiStream *stream);
/* Close if needed and free. No other call may be using the stream. */
void mi_stream_free(MiStream *stream);
void mi_frame_free(MiFrame *frame);

#ifdef __cplusplus
}
#endif

#endif /* MEROS_INTEGRATIONS_H */
