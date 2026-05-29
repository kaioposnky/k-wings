/// Minimal little-endian NBT parser for Bedrock Edition level.dat.
/// Implements the same format as pocketmine/nbt LittleEndianNbtSerializer.
///
/// Bedrock NBT wire format:
///   1 byte  — tag type (0x0A = Compound)
///   2 bytes LE — name length
///   N bytes — name
///   compound payload: repeated named tags until TAG_End (0x00)
use std::collections::HashMap;
use std::io::{Cursor, Read};
use anyhow::{bail, Context, Result};

const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_BYTE_ARRAY: u8 = 7;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;
const TAG_INT_ARRAY: u8 = 11;
const TAG_LONG_ARRAY: u8 = 12;

#[derive(Debug, Clone)]
pub enum NbtValue {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(u8, Vec<NbtValue>),
    Compound(HashMap<String, NbtValue>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

/// Parse Bedrock NBT: reads type byte + name + compound payload.
pub fn parse_root(data: &[u8]) -> Result<HashMap<String, NbtValue>> {
    let mut c = Cursor::new(data);
    let tag_type = read_u8(&mut c).context("Failed to read root tag type")?;
    if tag_type != TAG_COMPOUND {
        bail!("Root NBT tag is not a Compound (got type {})", tag_type);
    }
    let _name = read_string(&mut c).context("Failed to read root tag name")?;
    read_compound_payload(&mut c)
}

/// Serialize back to Bedrock NBT (type byte + empty name + compound payload).
pub fn serialize_root(map: &HashMap<String, NbtValue>) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    buf.push(TAG_COMPOUND);
    write_string(&mut buf, "");
    write_compound_payload(&mut buf, map)?;
    Ok(buf)
}

// ---- Read helpers ----

fn read_u8(c: &mut Cursor<&[u8]>) -> Result<u8> {
    let mut b = [0u8; 1];
    c.read_exact(&mut b)?;
    Ok(b[0])
}

fn read_le_u16(c: &mut Cursor<&[u8]>) -> Result<u16> {
    let mut b = [0u8; 2];
    c.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn read_le_i16(c: &mut Cursor<&[u8]>) -> Result<i16> {
    let mut b = [0u8; 2];
    c.read_exact(&mut b)?;
    Ok(i16::from_le_bytes(b))
}

fn read_le_i32(c: &mut Cursor<&[u8]>) -> Result<i32> {
    let mut b = [0u8; 4];
    c.read_exact(&mut b)?;
    Ok(i32::from_le_bytes(b))
}

fn read_le_i64(c: &mut Cursor<&[u8]>) -> Result<i64> {
    let mut b = [0u8; 8];
    c.read_exact(&mut b)?;
    Ok(i64::from_le_bytes(b))
}

fn read_le_f32(c: &mut Cursor<&[u8]>) -> Result<f32> {
    let mut b = [0u8; 4];
    c.read_exact(&mut b)?;
    Ok(f32::from_le_bytes(b))
}

fn read_le_f64(c: &mut Cursor<&[u8]>) -> Result<f64> {
    let mut b = [0u8; 8];
    c.read_exact(&mut b)?;
    Ok(f64::from_le_bytes(b))
}

fn read_string(c: &mut Cursor<&[u8]>) -> Result<String> {
    let len = read_le_u16(c)? as usize;
    let mut bytes = vec![0u8; len];
    c.read_exact(&mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

fn read_payload(c: &mut Cursor<&[u8]>, tag_type: u8) -> Result<NbtValue> {
    match tag_type {
        TAG_BYTE => Ok(NbtValue::Byte(read_u8(c)? as i8)),
        TAG_SHORT => Ok(NbtValue::Short(read_le_i16(c)?)),
        TAG_INT => Ok(NbtValue::Int(read_le_i32(c)?)),
        TAG_LONG => Ok(NbtValue::Long(read_le_i64(c)?)),
        TAG_FLOAT => Ok(NbtValue::Float(read_le_f32(c)?)),
        TAG_DOUBLE => Ok(NbtValue::Double(read_le_f64(c)?)),
        TAG_BYTE_ARRAY => {
            let len = read_le_i32(c)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_u8(c)? as i8);
            }
            Ok(NbtValue::ByteArray(arr))
        }
        TAG_STRING => Ok(NbtValue::String(read_string(c)?)),
        TAG_LIST => {
            let elem_type = read_u8(c)?;
            let count = read_le_i32(c)?;
            let count = if count < 0 { 0 } else { count as usize };
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                if elem_type == TAG_END {
                    break;
                }
                list.push(read_payload(c, elem_type)?);
            }
            Ok(NbtValue::List(elem_type, list))
        }
        TAG_COMPOUND => {
            let map = read_compound_payload(c)?;
            Ok(NbtValue::Compound(map))
        }
        TAG_INT_ARRAY => {
            let len = read_le_i32(c)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_le_i32(c)?);
            }
            Ok(NbtValue::IntArray(arr))
        }
        TAG_LONG_ARRAY => {
            let len = read_le_i32(c)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_le_i64(c)?);
            }
            Ok(NbtValue::LongArray(arr))
        }
        t => bail!("Unknown NBT tag type: {}", t),
    }
}

fn read_compound_payload(c: &mut Cursor<&[u8]>) -> Result<HashMap<String, NbtValue>> {
    let mut map = HashMap::new();
    loop {
        let tag_type = read_u8(c)?;
        if tag_type == TAG_END {
            break;
        }
        let name = read_string(c)?;
        let value = read_payload(c, tag_type)?;
        map.insert(name, value);
    }
    Ok(map)
}

// ---- Write helpers ----

fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

fn write_le_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_le_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_le_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_le_i64(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_le_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_le_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_le_u16(buf, bytes.len() as u16);
    buf.extend_from_slice(bytes);
}

fn tag_type_of(v: &NbtValue) -> u8 {
    match v {
        NbtValue::Byte(_) => TAG_BYTE,
        NbtValue::Short(_) => TAG_SHORT,
        NbtValue::Int(_) => TAG_INT,
        NbtValue::Long(_) => TAG_LONG,
        NbtValue::Float(_) => TAG_FLOAT,
        NbtValue::Double(_) => TAG_DOUBLE,
        NbtValue::ByteArray(_) => TAG_BYTE_ARRAY,
        NbtValue::String(_) => TAG_STRING,
        NbtValue::List(_, _) => TAG_LIST,
        NbtValue::Compound(_) => TAG_COMPOUND,
        NbtValue::IntArray(_) => TAG_INT_ARRAY,
        NbtValue::LongArray(_) => TAG_LONG_ARRAY,
    }
}

fn write_payload(buf: &mut Vec<u8>, v: &NbtValue) -> Result<()> {
    match v {
        NbtValue::Byte(b) => write_u8(buf, *b as u8),
        NbtValue::Short(s) => write_le_i16(buf, *s),
        NbtValue::Int(i) => write_le_i32(buf, *i),
        NbtValue::Long(l) => write_le_i64(buf, *l),
        NbtValue::Float(f) => write_le_f32(buf, *f),
        NbtValue::Double(d) => write_le_f64(buf, *d),
        NbtValue::ByteArray(arr) => {
            write_le_i32(buf, arr.len() as i32);
            for b in arr {
                write_u8(buf, *b as u8);
            }
        }
        NbtValue::String(s) => write_string(buf, s),
        NbtValue::List(elem_type, items) => {
            write_u8(buf, *elem_type);
            write_le_i32(buf, items.len() as i32);
            for item in items {
                write_payload(buf, item)?;
            }
        }
        NbtValue::Compound(map) => write_compound_payload(buf, map)?,
        NbtValue::IntArray(arr) => {
            write_le_i32(buf, arr.len() as i32);
            for i in arr {
                write_le_i32(buf, *i);
            }
        }
        NbtValue::LongArray(arr) => {
            write_le_i32(buf, arr.len() as i32);
            for l in arr {
                write_le_i64(buf, *l);
            }
        }
    }
    Ok(())
}

fn write_compound_payload(buf: &mut Vec<u8>, map: &HashMap<String, NbtValue>) -> Result<()> {
    for (name, value) in map {
        write_u8(buf, tag_type_of(value));
        write_string(buf, name);
        write_payload(buf, value)?;
    }
    write_u8(buf, TAG_END);
    Ok(())
}
