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

/* Returns 0 on success. The result must be released with xca_free. */
int32_t xca_compress(const uint8_t *data, size_t len, uint8_t level, XcaBuffer *output);
int32_t xca_decompress(const uint8_t *data, size_t len, XcaBuffer *output);
void xca_free(uint8_t *data, size_t len);

#ifdef __cplusplus
}
#endif

#endif
