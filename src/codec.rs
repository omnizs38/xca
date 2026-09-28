pub(crate) fn crc32(input:&[u8])->u32{let mut crc=u32::MAX;for &byte in input{crc^=byte as u32;for _ in 0..8{let mask=0u32.wrapping_sub(crc&1);crc=(crc>>1)^(0xedb8_8320&mask);}}!crc}
