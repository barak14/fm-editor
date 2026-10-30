//! Debug MCP tool implementations — one module per logical group.

pub mod attach;
pub mod entities;
pub mod fields_read;
pub mod logs;
pub mod memory;
pub mod name_debug;
pub mod pointers;
pub mod schema;
pub mod witness;

// ---------------------------------------------------------------------------
// Shared encoding helpers used by memory, witness, and probe tools.
// ---------------------------------------------------------------------------

use crate::process::FmProcess;

/// Wire encoding for MCP tool parameters.
#[derive(Clone, Copy)]
pub enum Encoding {
    U8,
    U16Le,
    U32Le,
    U64Le,
    I8,
    I16Le,
    I32Le,
    I64Le,
    F32Le,
    F64Le,
    FmString,
    BoolU8,
}

impl Encoding {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "u8"       => Ok(Self::U8),
            "u16le"    => Ok(Self::U16Le),
            "u32le"    => Ok(Self::U32Le),
            "u64le"    => Ok(Self::U64Le),
            "i8"       => Ok(Self::I8),
            "i16le"    => Ok(Self::I16Le),
            "i32le"    => Ok(Self::I32Le),
            "i64le"    => Ok(Self::I64Le),
            "f32le"    => Ok(Self::F32Le),
            "f64le"    => Ok(Self::F64Le),
            "fm_string"=> Ok(Self::FmString),
            "bool_u8"  => Ok(Self::BoolU8),
            other      => Err(format!("Unknown encoding '{other}'. Use: u8|u16le|u32le|u64le|i8|i16le|i32le|i64le|f32le|f64le|fm_string|bool_u8")),
        }
    }

    /// Byte width of one value in this encoding.
    pub fn size(self) -> usize {
        match self {
            Self::U8 | Self::I8 | Self::BoolU8 => 1,
            Self::U16Le | Self::I16Le           => 2,
            Self::U32Le | Self::I32Le | Self::F32Le => 4,
            Self::U64Le | Self::I64Le | Self::F64Le | Self::FmString => 8,
        }
    }

    /// Decode a byte slice to a display string. For FmString, dereferences via fm.
    pub fn decode(self, raw: &[u8], fm: &FmProcess) -> String {
        match self {
            Self::U8    if raw.len() >= 1 => raw[0].to_string(),
            Self::I8    if raw.len() >= 1 => (raw[0] as i8).to_string(),
            Self::BoolU8 if raw.len() >= 1 => if raw[0] != 0 { "true".to_string() } else { "false".to_string() },
            Self::U16Le if raw.len() >= 2 => u16::from_le_bytes(raw[..2].try_into().unwrap()).to_string(),
            Self::I16Le if raw.len() >= 2 => i16::from_le_bytes(raw[..2].try_into().unwrap()).to_string(),
            Self::U32Le if raw.len() >= 4 => u32::from_le_bytes(raw[..4].try_into().unwrap()).to_string(),
            Self::I32Le if raw.len() >= 4 => i32::from_le_bytes(raw[..4].try_into().unwrap()).to_string(),
            Self::U64Le if raw.len() >= 8 => u64::from_le_bytes(raw[..8].try_into().unwrap()).to_string(),
            Self::I64Le if raw.len() >= 8 => i64::from_le_bytes(raw[..8].try_into().unwrap()).to_string(),
            Self::F32Le if raw.len() >= 4 => f32::from_le_bytes(raw[..4].try_into().unwrap()).to_string(),
            Self::F64Le if raw.len() >= 8 => f64::from_le_bytes(raw[..8].try_into().unwrap()).to_string(),
            Self::FmString if raw.len() >= 8 => {
                let ptr = u64::from_le_bytes(raw[..8].try_into().unwrap());
                fm.read_fm_name(ptr)
                    .unwrap_or_else(|_| format!("(invalid ptr 0x{ptr:X})"))
            }
            _ => "(insufficient bytes)".to_string(),
        }
    }

    /// Hex representation of the raw bytes.
    pub fn raw_hex(raw: &[u8], size: usize) -> String {
        raw.iter()
            .take(size)
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Extract an integer argument from JSON, allowing both integer and string.
pub fn arg_u64(args: &serde_json::Value, key: &str) -> Result<u64, String> {
    let v = args.get(key).ok_or_else(|| format!("missing argument '{key}'"))?;
    if let Some(n) = v.as_u64() { return Ok(n); }
    if let Some(s) = v.as_str() {
        let s = s.trim_start_matches("0x").trim_start_matches("0X");
        return u64::from_str_radix(s, 16)
            .or_else(|_| s.parse::<u64>())
            .map_err(|_| format!("cannot parse '{key}' as integer: {v}"));
    }
    Err(format!("argument '{key}' must be an integer, got: {v}"))
}

pub fn arg_usize(args: &serde_json::Value, key: &str) -> Result<usize, String> {
    arg_u64(args, key).map(|n| n as usize)
}
