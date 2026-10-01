const fn crc32_tables() -> [[u32; 256]; 8] {
    let mut tables = [[0u32; 256]; 8];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xedb8_8320
            } else {
                value >> 1
            };
            bit += 1;
        }
        tables[0][index] = value;
        index += 1;
    }
    let mut table = 1;
    while table < tables.len() {
        index = 0;
        while index < 256 {
            let previous = tables[table - 1][index];
            tables[table][index] = tables[0][(previous & 0xff) as usize] ^ (previous >> 8);
            index += 1;
        }
        table += 1;
    }
    tables
}

const CRC32_TABLES: [[u32; 256]; 8] = crc32_tables();

pub(crate) fn crc32(input: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    let (chunks, remainder) = input.as_chunks::<8>();
    for chunk in chunks {
        crc ^= u32::from_le_bytes(chunk[..4].try_into().unwrap());
        crc = CRC32_TABLES[7][(crc & 0xff) as usize]
            ^ CRC32_TABLES[6][((crc >> 8) & 0xff) as usize]
            ^ CRC32_TABLES[5][((crc >> 16) & 0xff) as usize]
            ^ CRC32_TABLES[4][(crc >> 24) as usize]
            ^ CRC32_TABLES[3][chunk[4] as usize]
            ^ CRC32_TABLES[2][chunk[5] as usize]
            ^ CRC32_TABLES[1][chunk[6] as usize]
            ^ CRC32_TABLES[0][chunk[7] as usize];
    }
    for &byte in remainder {
        crc = CRC32_TABLES[0][((crc ^ byte as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytewise(input: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for &byte in input {
            crc = CRC32_TABLES[0][((crc ^ byte as u32) & 0xff) as usize] ^ (crc >> 8);
        }
        !crc
    }

    #[test]
    fn crc32_matches_standard_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn sliced_crc_matches_bytewise_at_boundaries() {
        let data: Vec<u8> = (0..4097).map(|index| (index * 37) as u8).collect();
        for length in 0..=data.len() {
            assert_eq!(crc32(&data[..length]), bytewise(&data[..length]));
        }
    }
}
