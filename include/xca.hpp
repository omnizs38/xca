#ifndef XCA_HPP
#define XCA_HPP

#include "xca.h"
#include <cstdint>
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

inline std::vector<std::uint8_t> decompress(const std::uint8_t *data, std::size_t size) {
    XcaBuffer output{nullptr, 0};
    const auto code = xca_decompress(data, size, &output);
    if (code != 0) throw std::runtime_error(xca_error_string(code));
    std::vector<std::uint8_t> result(output.data, output.data + output.len);
    xca_buffer_free(&output);
    return result;
}

inline std::vector<std::uint8_t> compress(const std::vector<std::uint8_t> &data) {
    return compress(data.data(), data.size());
}

inline std::vector<std::uint8_t> decompress(const std::vector<std::uint8_t> &data) {
    return decompress(data.data(), data.size());
}

} // namespace xca

#endif
