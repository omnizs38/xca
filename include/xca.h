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

/*
 * Returns 0 on success. Codes 1-6 are described by xca_error_string.
 * Output buffers are cleared before every call and must be released with
 * xca_buffer_free or xca_free. Empty successful outputs use {NULL, 0}.
 */
int32_t xca_compress(const uint8_t *data, size_t len, XcaBuffer *output);
int32_t xca_decompress(const uint8_t *data, size_t len, XcaBuffer *output);
int32_t xca_decompress_with_limit(const uint8_t *data, size_t len, size_t max_output, XcaBuffer *output);
int32_t xca_decompressed_size(const uint8_t *data, size_t len, size_t *output_len);
int32_t xca_decompressed_size_with_limit(
    const uint8_t *data,
    size_t len,
    size_t max_output,
    size_t *output_len);
int32_t xca_decompress_into(const uint8_t *data, size_t len, uint8_t *output, size_t output_len);
int32_t xca_decompress_into_with_limit(const uint8_t *data, size_t len, uint8_t *output, size_t output_len, size_t max_output);
void xca_free(uint8_t *data, size_t len);
void xca_buffer_free(XcaBuffer *buffer);

#ifdef __cplusplus
}
#endif

#endif
