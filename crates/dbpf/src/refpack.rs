use crate::error::{Error, Result};

/// Parse the RefPack stream header (2 flag bytes + 3/4-byte big-endian size,
/// doubled when the multi-stream bit is set). Returns the declared
/// decompressed size and the number of header bytes consumed.
///
/// Bit layout of the first byte (mirrors `ReadRefPackCompressionHeader`):
/// - `0x80` — long (4-byte) size instead of 3-byte
/// - `0x01` — a second (ignored) size block follows (multi-stream)
/// - `(b & 0x3E) == 0x10` and second byte `0xFB` identify the format
pub fn parse_stream_header(input: &[u8]) -> Result<(u32, usize)> {
    if input.len() < 2 {
        return Err(Error::Truncated {
            needed: 2,
            at: 0,
            size: input.len(),
        });
    }
    let (b0, b1) = (input[0], input[1]);
    if (b0 & 0x3E) != 0x10 || b1 != 0xFB {
        return Err(Error::Refpack("bad stream header"));
    }
    let is_long = b0 & 0x80 != 0;
    let has_more = b0 & 0x01 != 0;
    let extra = (if is_long { 4 } else { 3 }) * (if has_more { 2 } else { 1 });
    let total = 2 + extra;
    if input.len() < total {
        return Err(Error::Truncated {
            needed: total,
            at: 0,
            size: input.len(),
        });
    }

    let d = &input[2..];
    let mut size: u32 = (u32::from(d[0]) << 16) | (u32::from(d[1]) << 8) | u32::from(d[2]);
    if is_long {
        size = (size << 8) | u32::from(d[3]);
    }
    Ok((size, total))
}

/// Decompress a complete RefPack blob into a preallocated buffer of exactly
/// `decompressed_size` bytes. The size normally comes from the package index
/// entry, so the output never reallocates mid-loop.
///
/// Port of `StreamHelpers.RefPackDecompress`: copy commands reproduce
/// overlaps by copying byte-wise from the output written so far, and the
/// stream is bounded by the blob length (the C# loop's compressedSize).
pub fn decompress(input: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
    let (_, header_len) = parse_stream_header(input)?;

    let mut out = vec![0u8; decompressed_size];
    let mut pos = header_len;
    let mut off = 0usize;

    while pos < input.len() {
        let mut copy_size = 0usize;
        let mut copy_offset = 0usize;
        let mut stop = false;

        let prefix = input[pos];
        pos += 1;

        let plain_size: usize = if prefix >= 0xC0 {
            if prefix >= 0xE0 {
                if prefix >= 0xFC {
                    stop = true;
                    usize::from(prefix & 0x03)
                } else {
                    (usize::from(prefix & 0x1F) + 1) * 4
                }
            } else {
                let e = take(input, &mut pos, 3)?;
                copy_size = ((usize::from(prefix & 0x0C) << 6) | usize::from(e[2])) + 5;
                copy_offset = ((((usize::from(prefix & 0x10) << 4) | usize::from(e[0])) << 8)
                    | usize::from(e[1]))
                    + 1;
                usize::from(prefix & 0x03)
            }
        } else if prefix >= 0x80 {
            let e = take(input, &mut pos, 2)?;
            copy_size = usize::from(prefix & 0x3F) + 4;
            copy_offset = ((usize::from(e[0] & 0x3F) << 8) | usize::from(e[1])) + 1;
            usize::from(e[0] >> 6)
        } else {
            let e = take(input, &mut pos, 1)?;
            copy_size = (usize::from((prefix & 0x1C) >> 2)) + 3;
            copy_offset = ((usize::from(prefix & 0x60) << 3) | usize::from(e[0])) + 1;
            usize::from(prefix & 0x03)
        };

        if plain_size > 0 {
            let src = take(input, &mut pos, plain_size)?;
            if off + plain_size > out.len() {
                return Err(Error::Refpack("literal overruns output"));
            }
            out[off..off + plain_size].copy_from_slice(src);
            off += plain_size;
        }

        if copy_size > 0 {
            if copy_offset > off {
                return Err(Error::Refpack("copy references unwritten data"));
            }
            if off + copy_size > out.len() {
                return Err(Error::Refpack("copy overruns output"));
            }
            for i in 0..copy_size {
                out[off + i] = out[off - copy_offset + i];
            }
            off += copy_size;
        }

        if stop {
            break;
        }
    }

    if off != out.len() {
        return Err(Error::Refpack("decompressed size mismatch"));
    }
    Ok(out)
}

fn take<'a>(input: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = *pos + n;
    let slice = input
        .get(*pos..end)
        .ok_or(Error::Refpack("stream truncated"))?;
    *pos = end;
    Ok(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(size: u32) -> Vec<u8> {
        vec![
            0x10,
            0xFB,
            (size >> 16) as u8,
            (size >> 8) as u8,
            size as u8,
        ]
    }

    fn decompress_ok(stream: &[u8], expected: &[u8]) {
        let out = decompress(stream, expected.len()).expect("decompress");
        assert_eq!(out, expected);
    }

    #[test]
    fn literals_only() {
        // 0xE1: 8 literals, then 0xFC stop with no literals.
        let mut s = header(8);
        s.push(0xE1);
        s.extend_from_slice(b"ABCDEFGH");
        s.push(0xFC);
        decompress_ok(&s, b"ABCDEFGH");
    }

    #[test]
    fn stop_command_carries_trailing_literals() {
        // 0xE0: 4 literals, 0xFD stop carrying 1 more literal; junk after is
        // ignored because the loop breaks at stop.
        let mut s = header(5);
        s.push(0xE0);
        s.extend_from_slice(b"ABCD");
        s.push(0xFD);
        s.push(b'E');
        s.extend_from_slice(&[0xFF, 0xFF]); // must not be read
        decompress_ok(&s, b"ABCDE");
    }

    #[test]
    fn short_command_overlap_copy() {
        // prefix 0x06: 2 literals "AB", copy 4 bytes from offset 2 -> ABABAB
        let mut s = header(6);
        s.extend_from_slice(&[0x06, 0x01, b'A', b'B']);
        s.push(0xFC);
        decompress_ok(&s, b"ABABAB");
    }

    #[test]
    fn medium_command_run() {
        // prefix 0x80, extra 0xC0 0x00: 3 literals "XYZ", copy 4 from offset 1
        let mut s = header(7);
        s.extend_from_slice(&[0x80, 0xC0, 0x00, b'X', b'Y', b'Z']);
        s.push(0xFC);
        decompress_ok(&s, b"XYZZZZZ");
    }

    #[test]
    fn large_command_run() {
        // prefix 0xC1 + extra 00 00 00: 1 literal "W" (read after the extra
        // bytes), copy 5 bytes from offset 1
        let mut s = header(6);
        s.extend_from_slice(&[0xC1, 0x00, 0x00, 0x00, b'W']);
        s.push(0xFC);
        decompress_ok(&s, b"WWWWWW");
    }

    #[test]
    fn extended_literal_run() {
        // 0xE3: ((0x03)+1)*4 = 16 literals
        let mut s = header(16);
        s.push(0xE3);
        s.extend_from_slice(b"0123456789ABCDEF");
        s.push(0xFC);
        decompress_ok(&s, b"0123456789ABCDEF");
    }

    #[test]
    fn multi_stream_header_skips_second_size_block() {
        // 0x11 sets the has-more bit: 6 size bytes follow the 2 magic bytes,
        // declared size comes from the first 3. Payload starts at byte 8.
        let mut s = vec![0x11, 0xFB, 0x00, 0x00, 0x04, 0xAA, 0xBB, 0xCC];
        let (declared, header_len) = parse_stream_header(&s).unwrap();
        assert_eq!(declared, 4);
        assert_eq!(header_len, 8);
        s.push(0xE0); // 4 literals
        s.extend_from_slice(b"ABCD");
        s.push(0xFC);
        decompress_ok(&s, b"ABCD");
    }

    #[test]
    fn long_size_header() {
        // 0x90 sets the long bit: 4-byte big-endian declared size (4 here).
        let mut s = vec![0x90, 0xFB, 0x00, 0x00, 0x00, 0x04];
        let (declared, header_len) = parse_stream_header(&s).unwrap();
        assert_eq!(declared, 4);
        assert_eq!(header_len, 6);
        s.push(0xE0);
        s.extend_from_slice(b"XYZW");
        s.push(0xFC);
        decompress_ok(&s, b"XYZW");
    }

    #[test]
    fn bad_magic_rejected() {
        let s = [0x12, 0x34, 0x00, 0x00, 0x00];
        assert!(matches!(decompress(&s, 4), Err(Error::Refpack(_))));
    }

    #[test]
    fn size_mismatch_detected() {
        let mut s = header(8);
        s.push(0xE1);
        s.extend_from_slice(b"ABCD"); // only 4 of 8 literals, then EOF
        assert!(matches!(decompress(&s, 8), Err(Error::Refpack(_))));
    }
}
