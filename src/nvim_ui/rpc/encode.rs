//! Direct MessagePack-RPC notification encoders for UI input.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    StringTooLong,
}

fn push_array_len(out: &mut Vec<u8>, len: usize) -> Result<(), EncodeError> {
    if len < 16 {
        out.push(0x90 | len as u8);
        return Ok(());
    }
    if len <= u16::MAX as usize {
        out.push(0xdc);
        out.extend((len as u16).to_be_bytes());
        return Ok(());
    }
    if len <= u32::MAX as usize {
        out.push(0xdd);
        out.extend((len as u32).to_be_bytes());
        return Ok(());
    }
    Err(EncodeError::StringTooLong)
}

fn push_str(out: &mut Vec<u8>, value: &str) -> Result<(), EncodeError> {
    let len = value.len();
    if len < 32 {
        out.push(0xa0 | len as u8);
    } else if len <= u8::MAX as usize {
        out.extend([0xd9, len as u8]);
    } else if len <= u16::MAX as usize {
        out.push(0xda);
        out.extend((len as u16).to_be_bytes());
    } else if len <= u32::MAX as usize {
        out.push(0xdb);
        out.extend((len as u32).to_be_bytes());
    } else {
        return Err(EncodeError::StringTooLong);
    }
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_uint(out: &mut Vec<u8>, value: u64) {
    if value <= 0x7f {
        out.push(value as u8);
    } else if value <= u8::MAX as u64 {
        out.extend([0xcc, value as u8]);
    } else if value <= u16::MAX as u64 {
        out.push(0xcd);
        out.extend((value as u16).to_be_bytes());
    } else if value <= u32::MAX as u64 {
        out.push(0xce);
        out.extend((value as u32).to_be_bytes());
    } else {
        out.push(0xcf);
        out.extend(value.to_be_bytes());
    }
}

fn push_int(out: &mut Vec<u8>, value: i64) {
    if value >= 0 {
        return push_uint(out, value as u64);
    }
    if value >= -32 {
        out.push(value as i8 as u8);
    } else if value >= i8::MIN as i64 {
        out.extend([0xd0, value as i8 as u8]);
    } else if value >= i16::MIN as i64 {
        out.push(0xd1);
        out.extend((value as i16).to_be_bytes());
    } else if value >= i32::MIN as i64 {
        out.push(0xd2);
        out.extend((value as i32).to_be_bytes());
    } else {
        out.push(0xd3);
        out.extend(value.to_be_bytes());
    }
}

fn notification(out: &mut Vec<u8>, method: &str, params: usize) -> Result<(), EncodeError> {
    push_array_len(out, 3)?;
    out.push(2);
    push_str(out, method)?;
    push_array_len(out, params)
}

pub fn encode_nvim_input(input: &str) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    encode_nvim_input_into(&mut out, input)?;
    Ok(out)
}

pub fn encode_nvim_input_into(out: &mut Vec<u8>, input: &str) -> Result<(), EncodeError> {
    notification(out, "nvim_input", 1)?;
    push_str(out, input)
}

pub fn encode_nvim_input_mouse(
    button: &str,
    action: &str,
    modifier: &str,
    grid: i64,
    row: i64,
    col: i64,
) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    encode_nvim_input_mouse_into(&mut out, button, action, modifier, grid, row, col)?;
    Ok(out)
}

pub fn encode_nvim_input_mouse_into(
    out: &mut Vec<u8>,
    button: &str,
    action: &str,
    modifier: &str,
    grid: i64,
    row: i64,
    col: i64,
) -> Result<(), EncodeError> {
    notification(out, "nvim_input_mouse", 6)?;
    push_str(out, button)?;
    push_str(out, action)?;
    push_str(out, modifier)?;
    push_int(out, grid);
    push_int(out, row);
    push_int(out, col);
    Ok(())
}

pub fn encode_nvim_ui_try_resize(width: u64, height: u64) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    encode_nvim_ui_try_resize_into(&mut out, width, height)?;
    Ok(out)
}

pub fn encode_nvim_ui_try_resize_into(
    out: &mut Vec<u8>,
    width: u64,
    height: u64,
) -> Result<(), EncodeError> {
    notification(out, "nvim_ui_try_resize", 2)?;
    push_uint(out, width);
    push_uint(out, height);
    Ok(())
}

pub fn encode_nvim_paste(data: &str) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    encode_nvim_paste_into(&mut out, data)?;
    Ok(out)
}

pub fn encode_nvim_paste_into(out: &mut Vec<u8>, data: &str) -> Result<(), EncodeError> {
    notification(out, "nvim_paste", 3)?;
    push_str(out, data)?;
    out.extend([0xc2, 0xff]);
    Ok(())
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod encode_tests;
