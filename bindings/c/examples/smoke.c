/* Builds against meros_integrations.h and the static library, and exercises
 * the interface from C: start a core, read the catalogue, refuse a bad open
 * and a stream of a device that is not open, poll events, and free
 * everything it was given. */
#include <stdio.h>
#include <string.h>
#include "meros_integrations.h"

int main(void) {
    MiCore *core = mi_core_new();
    if (!core) { fprintf(stderr, "core did not start\n"); return 1; }

    char *catalog = mi_catalog(core);
    int has_kramer = strstr(catalog, "\"kramer-p3000\"") != NULL;
    mi_string_free(catalog);

    char *refused = mi_open(core, "{\"device\":\"no-such-device\",\"model\":\"x\",\"host\":\"127.0.0.1\"}");
    int is_error = strstr(refused, "\"unknown_device\"") != NULL;
    printf("%s\n", refused);
    mi_string_free(refused);

    MiStream *stream = NULL;
    char *no_stream = mi_stream_open(core, 999, "live", &stream);
    int stream_refused = stream == NULL && strstr(no_stream, "\"closed\"") != NULL;
    mi_string_free(no_stream);
    mi_stream_free(stream);

    char *events = mi_poll_events(core, 16);
    mi_string_free(events);

    mi_core_free(core);
    if (!has_kramer || !is_error || !stream_refused) { fprintf(stderr, "unexpected result\n"); return 1; }
    printf("ok\n");
    return 0;
}
