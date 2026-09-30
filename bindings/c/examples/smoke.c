/* Builds against meros_integrations.h and the static library, and exercises
 * the interface from C: start a core, read the catalogue, refuse a bad open,
 * poll events, and free everything it was given. */
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

    char *events = mi_poll_events(core, 16);
    mi_string_free(events);

    mi_core_free(core);
    if (!has_kramer || !is_error) { fprintf(stderr, "unexpected result\n"); return 1; }
    printf("ok\n");
    return 0;
}
