#ifndef XCA_H
#define XCA_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct XcaBuffer {
    uint8_t *data;
    size_t len;
} XcaBuffer;

const char *xca_version(void);
const char *xca_error_string(int32_t code);

/* Returns 0 on success. The result must be released with xca_free. */
int32_t xca_compress(const uint8_t *data, size_t len, XcaBuffer *output);
int32_t xca_decompress(const uint8_t *data, size_t len, XcaBuffer *output);
void xca_free(uint8_t *data, size_t len);
void xca_buffer_free(XcaBuffer *buffer);

#ifdef __cplusplus
}
#endif

#endif
