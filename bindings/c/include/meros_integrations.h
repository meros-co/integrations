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

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MiCore MiCore;

/* Start a core, with its own threads. NULL if it cannot start. */
MiCore *mi_core_new(void);
/* options: {"bind_address":"<local interface address>"}, or NULL for defaults.
 * NULL if the options are invalid or the core cannot start. */
MiCore *mi_core_new_with_options(const char *options);
void mi_core_free(MiCore *core);
void mi_string_free(char *s);

/* Every device spec: models, commands, parameters, quirks. */
char *mi_catalog(const MiCore *core);

/* request: {"device":"<spec id>","model":"<model id>","host":"<ip or name>",
 *           "port":<optional>,"settings":{...}}
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

#ifdef __cplusplus
}
#endif

#endif /* MEROS_INTEGRATIONS_H */
