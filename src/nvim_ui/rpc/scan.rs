//! Allocation-free MessagePack marker scanning.

use std::str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    Incomplete,
    Malformed,
}

fn bytes<'a>(buf: &'a [u8], pos: &mut usize, len: usize) -> Result<&'a [u8], ScanError> {
    let end = pos.checked_add(len).ok_or(ScanError::Malformed)?;
    if end > buf.len() {
        return Err(ScanError::Incomplete);
    }
    let result = &buf[*pos..end];
    *pos = end;
    Ok(result)
}

fn length(buf: &[u8], pos: &mut usize, width: usize) -> Result<usize, ScanError> {
    let raw = bytes(buf, pos, width)?;
    let value = match width {
        1 => raw[0] as u64,
        2 => u16::from_be_bytes([raw[0], raw[1]]) as u64,
        4 => u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as u64,
        _ => return Err(ScanError::Malformed),
    };
    usize::try_from(value).map_err(|_| ScanError::Malformed)
}

fn skip_bytes(buf: &[u8], pos: &mut usize, len: usize) -> Result<(), ScanError> {
    let _ = bytes(buf, pos, len)?;
    Ok(())
}

fn skip_length(buf: &[u8], pos: &mut usize, width: usize, extra: usize) -> Result<(), ScanError> {
    let len = length(buf, pos, width)?
        .checked_add(extra)
        .ok_or(ScanError::Malformed)?;
    skip_bytes(buf, pos, len)
}

fn skip_at(buf: &[u8], pos: &mut usize) -> Result<(), ScanError> {
    let marker = bytes(buf, pos, 1)?[0];
    match marker {
        0x00..=0x7f | 0xe0..=0xff | 0xc0 | 0xc2..=0xc3 => Ok(()),
        0x80..=0x8f => {
            for _ in 0..(marker as usize & 0x0f) * 2 {
                skip_at(buf, pos)?;
            }
            Ok(())
        }
        0x90..=0x9f => {
            for _ in 0..(marker as usize & 0x0f) {
                skip_at(buf, pos)?;
            }
            Ok(())
        }
        0xa0..=0xbf => skip_bytes(buf, pos, marker as usize & 0x1f),
        0xc4 => skip_length(buf, pos, 1, 0),
        0xc5 => skip_length(buf, pos, 2, 0),
        0xc6 => skip_length(buf, pos, 4, 0),
        0xc7 => skip_length(buf, pos, 1, 1),
        0xc8 => skip_length(buf, pos, 2, 1),
        0xc9 => skip_length(buf, pos, 4, 1),
        0xca => skip_bytes(buf, pos, 4),
        0xcb => skip_bytes(buf, pos, 8),
        0xcc => skip_bytes(buf, pos, 1),
        0xcd => skip_bytes(buf, pos, 2),
        0xce => skip_bytes(buf, pos, 4),
        0xcf => skip_bytes(buf, pos, 8),
        0xd0 => skip_bytes(buf, pos, 1),
        0xd1 => skip_bytes(buf, pos, 2),
        0xd2 => skip_bytes(buf, pos, 4),
        0xd3 => skip_bytes(buf, pos, 8),
        0xd4 => skip_bytes(buf, pos, 2),
        0xd5 => skip_bytes(buf, pos, 3),
        0xd6 => skip_bytes(buf, pos, 5),
        0xd7 => skip_bytes(buf, pos, 9),
        0xd8 => skip_bytes(buf, pos, 17),
        0xd9 => skip_length(buf, pos, 1, 0),
        0xda => skip_length(buf, pos, 2, 0),
        0xdb => skip_length(buf, pos, 4, 0),
        0xdc => {
            for _ in 0..length(buf, pos, 2)? {
                skip_at(buf, pos)?;
            }
            Ok(())
        }
        0xdd => {
            for _ in 0..length(buf, pos, 4)? {
                skip_at(buf, pos)?;
            }
            Ok(())
        }
        0xde => {
            let count = length(buf, pos, 2)?;
            for _ in 0..count {
                skip_at(buf, pos)?;
                skip_at(buf, pos)?;
            }
            Ok(())
        }
        0xdf => {
            let count = length(buf, pos, 4)?;
            for _ in 0..count {
                skip_at(buf, pos)?;
                skip_at(buf, pos)?;
            }
            Ok(())
        }
        0xc1 => Err(ScanError::Malformed),
    }
}

pub fn skip_value(buf: &[u8], cursor: &mut usize) -> Result<(), ScanError> {
    let mut pos = *cursor;
    skip_at(buf, &mut pos)?;
    *cursor = pos;
    Ok(())
}

pub fn read_array_len(buf: &[u8], cursor: &mut usize) -> Result<usize, ScanError> {
    let mut pos = *cursor;
    let marker = bytes(buf, &mut pos, 1)?[0];
    let count = match marker {
        0x90..=0x9f => marker as usize & 0x0f,
        0xdc => length(buf, &mut pos, 2)?,
        0xdd => length(buf, &mut pos, 4)?,
        _ => return Err(ScanError::Malformed),
    };
    *cursor = pos;
    Ok(count)
}

pub fn read_uint(buf: &[u8], cursor: &mut usize) -> Result<u64, ScanError> {
    let mut pos = *cursor;
    let marker = bytes(buf, &mut pos, 1)?[0];
    let value = match marker {
        0x00..=0x7f => marker as u64,
        0xcc => bytes(buf, &mut pos, 1)?[0] as u64,
        0xcd => u16::from_be_bytes(
            bytes(buf, &mut pos, 2)?
                .try_into()
                .map_err(|_| ScanError::Malformed)?,
        ) as u64,
        0xce => u32::from_be_bytes(
            bytes(buf, &mut pos, 4)?
                .try_into()
                .map_err(|_| ScanError::Malformed)?,
        ) as u64,
        0xcf => u64::from_be_bytes(
            bytes(buf, &mut pos, 8)?
                .try_into()
                .map_err(|_| ScanError::Malformed)?,
        ),
        _ => return Err(ScanError::Malformed),
    };
    *cursor = pos;
    Ok(value)
}

pub fn read_str_slice<'a>(buf: &'a [u8], cursor: &mut usize) -> Result<&'a str, ScanError> {
    let mut pos = *cursor;
    let marker = bytes(buf, &mut pos, 1)?[0];
    let len = match marker {
        0xa0..=0xbf => marker as usize & 0x1f,
        0xd9 => length(buf, &mut pos, 1)?,
        0xda => length(buf, &mut pos, 2)?,
        0xdb => length(buf, &mut pos, 4)?,
        _ => return Err(ScanError::Malformed),
    };
    let raw = bytes(buf, &mut pos, len)?;
    let result = str::from_utf8(raw).map_err(|_| ScanError::Malformed)?;
    *cursor = pos;
    Ok(result)
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod scan_tests;
