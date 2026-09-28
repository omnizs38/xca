#include "xca.h"
#include <stdio.h>
#include <string.h>

int main(void) {
    const char *text = "XCA universal integration example";
    XcaBuffer compressed = {0};
    XcaBuffer restored = {0};

    int32_t code = xca_compress((const uint8_t *)text, strlen(text), &compressed);
    if (code != 0) {
        fprintf(stderr, "compress: %s\n", xca_error_string(code));
        return 1;
    }
    code = xca_decompress(compressed.data, compressed.len, &restored);
    if (code != 0) {
        fprintf(stderr, "decompress: %s\n", xca_error_string(code));
        xca_buffer_free(&compressed);
        return 1;
    }

    printf("XCA %s: %zu -> %zu -> %zu bytes\n", xca_version(), strlen(text), compressed.len, restored.len);
    xca_buffer_free(&compressed);
    xca_buffer_free(&restored);
    return 0;
}
