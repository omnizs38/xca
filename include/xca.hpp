#ifndef XCA_HPP
#define XCA_HPP

#include "xca.h"
#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

namespace xca {

inline std::string version() { return xca_version(); }

inline std::vector<std::uint8_t> compress(const std::uint8_t *data, std::size_t size) {
    XcaBuffer output{nullptr, 0};
    const auto code = xca_compress(data, size, &output);
    if (code != 0) throw std::runtime_error(xca_error_string(code));
    std::vector<std::uint8_t> result(output.data, output.data + output.len);
    xca_buffer_free(&output);
    return result;
}

inline std::vector<std::uint8_t> decompress(
    const std::uint8_t *data,
    std::size_t size,
    std::size_t max_output_size = 256U * 1024U * 1024U) {
    std::size_t output_size = 0;
    auto code = xca_decompressed_size(data, size, &output_size);
    if (code != 0) throw std::runtime_error(xca_error_string(code));
    if (output_size > max_output_size) {
        throw std::length_error("decoded output exceeds the configured limit");
    }
    std::vector<std::uint8_t> result(output_size);
    code = xca_decompress_into_with_limit(
        data, size, result.data(), result.size(), max_output_size);
    if (code != 0) throw std::runtime_error(xca_error_string(code));
    return result;
}

inline std::vector<std::uint8_t> compress(const std::vector<std::uint8_t> &data) {
    return compress(data.data(), data.size());
}

inline std::vector<std::uint8_t> decompress(
    const std::vector<std::uint8_t> &data,
    std::size_t max_output_size = 256U * 1024U * 1024U) {
    return decompress(data.data(), data.size(), max_output_size);
}

} // namespace xca

#endif
