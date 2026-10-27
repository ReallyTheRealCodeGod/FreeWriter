//! On Windows, gives writer.exe its icon and its version details (the name
//! Explorer and Task Manager show). The resources are written straight into
//! a .res file, which both link.exe and lld take as it is, so no resource
//! compiler is needed.

use std::env;
use std::fs;
use std::path::PathBuf;

const ICON: &str = "assets/writer.ico";

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
/// English (United States), with Unicode text.
const LANGUAGE: u16 = 0x0409;
const CODE_PAGE: u16 = 1200;

fn main() {
    println!("cargo:rerun-if-changed={ICON}");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let icon = fs::read(ICON).unwrap_or_else(|e| panic!("can't read {ICON}: {e}"));
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("writer.res");
    fs::write(&out, resources(&icon)).unwrap();
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// The .res file: the icon images, the icon group that lists them, and the
/// version details.
fn resources(ico: &[u8]) -> Vec<u8> {
    assert!(ico.len() > 6 && ico[..4] == [0, 0, 1, 0], "{ICON} isn't an icon file");
    // A .res file starts with an empty entry that marks it as 32-bit.
    let mut res = vec![0, 0, 0, 0, 32, 0, 0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff, 0, 0];
    res.resize(32, 0);

    let count = u16::from_le_bytes([ico[4], ico[5]]);
    let mut group = Vec::new();
    for word in [0, 1, count] {
        group.extend(word.to_le_bytes());
    }
    for id in 1..=count {
        // Each icon file entry is 16 bytes; the group's entry has the same
        // first 12 and then the image's resource id instead of its offset.
        let entry = &ico[6 + 16 * (id as usize - 1)..][..16];
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        add(&mut res, RT_ICON, id, &ico[offset..offset + size]);
        group.extend(&entry[..12]);
        group.extend(id.to_le_bytes());
    }
    add(&mut res, RT_GROUP_ICON, 1, &group);
    add(&mut res, RT_VERSION, 1, &version_info());
    res
}

/// One resource: a 32-byte header naming its type and id, then its data, padded
/// to a 4-byte boundary.
fn add(res: &mut Vec<u8>, kind: u16, id: u16, data: &[u8]) {
    res.extend((data.len() as u32).to_le_bytes());
    res.extend(32u32.to_le_bytes());
    for word in [0xffff, kind, 0xffff, id] {
        res.extend(u16::to_le_bytes(word));
    }
    res.extend(0u32.to_le_bytes()); // data version
    res.extend(0x1030u16.to_le_bytes()); // memory flags, ignored by Windows
    res.extend(LANGUAGE.to_le_bytes());
    res.extend(0u32.to_le_bytes()); // version
    res.extend(0u32.to_le_bytes()); // characteristics
    res.extend(data);
    pad(res);
}

/// The version resource: fixed numbers, then the text Windows shows.
fn version_info() -> Vec<u8> {
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let part = |name: &str| env::var(name).unwrap().parse::<u32>().unwrap_or(0);
    let high = part("CARGO_PKG_VERSION_MAJOR") << 16 | part("CARGO_PKG_VERSION_MINOR");
    let low = part("CARGO_PKG_VERSION_PATCH") << 16;
    let mut fixed = Vec::new();
    let fields = [
        0xfeef04bd, // signature
        0x0001_0000,
        high, // file version
        low,
        high, // product version
        low,
        0x3f, // flags mask
        0,    // flags
        0x0004_0004, // for Windows NT
        1,           // an application
        0,
        0, // date
        0,
    ];
    for field in fields {
        fixed.extend(u32::to_le_bytes(field));
    }

    let texts = [
        ("FileDescription", "Writer"),
        ("FileVersion", version.as_str()),
        ("InternalName", "writer"),
        ("OriginalFilename", "writer.exe"),
        ("ProductName", "Writer"),
        ("ProductVersion", version.as_str()),
    ];
    let texts = texts.iter().map(|(key, value)| {
        let value = utf16(value);
        node(key, 1, &value, (value.len() / 2) as u16, Vec::new())
    });
    let table = node(&format!("{LANGUAGE:04X}{CODE_PAGE:04X}"), 1, &[], 0, texts.collect());
    let strings = node("StringFileInfo", 1, &[], 0, vec![table]);
    let mut translation = LANGUAGE.to_le_bytes().to_vec();
    translation.extend(CODE_PAGE.to_le_bytes());
    let var = node("Translation", 0, &translation, 4, Vec::new());
    let vars = node("VarFileInfo", 1, &[], 0, vec![var]);
    node("VS_VERSION_INFO", 0, &fixed, fixed.len() as u16, vec![strings, vars])
}

/// A block of the version resource: its length, its value's length (in
/// characters for text), its type (1 for text), its key, its value, and the
/// blocks inside it, each starting on a 4-byte boundary.
fn node(key: &str, kind: u16, value: &[u8], value_length: u16, children: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = vec![0; 6];
    out.extend(utf16(key));
    pad(&mut out);
    out.extend(value);
    for child in children {
        pad(&mut out);
        out.extend(child);
    }
    let length = out.len() as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    out[2..4].copy_from_slice(&value_length.to_le_bytes());
    out[4..6].copy_from_slice(&kind.to_le_bytes());
    out
}

/// Text as UTF-16 with its closing zero, the way resources store it.
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn pad(bytes: &mut Vec<u8>) {
    bytes.resize(bytes.len().next_multiple_of(4), 0);
}
