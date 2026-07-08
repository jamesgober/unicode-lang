//! Table generator for `unicode-lang`.
//!
//! Reads the Unicode Character Database text files from `dev/ucd/` and emits
//! `src/tables.rs`: the compact, sorted lookup tables the crate binary-searches
//! at runtime. This keeps the shipped crate dependency-free while remaining
//! correct-by-construction against the published UCD.
//!
//! This is a development tool, not part of the crate build. It is committed for
//! reproducibility and auditability. Regenerate with:
//!
//! ```text
//! # from the crate root, with dev/ucd/ populated (see dev/README or the
//! # curl commands in the v0.2.0 release notes):
//! rustc -O dev/gen_tables.rs -o dev/gen_tables && ./dev/gen_tables
//! ```
//!
//! Uses only `std`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

const UCD: &str = "dev/ucd";
const OUT: &str = "src/tables.rs";
const VERSION: (u8, u8, u8) = (16, 0, 0);

fn main() {
    let unicode_data = fs::read_to_string(format!("{UCD}/UnicodeData.txt")).expect("UnicodeData.txt");
    let derived_core = fs::read_to_string(format!("{UCD}/DerivedCoreProperties.txt")).expect("DerivedCoreProperties.txt");
    let derived_norm = fs::read_to_string(format!("{UCD}/DerivedNormalizationProps.txt")).expect("DerivedNormalizationProps.txt");
    let eaw = fs::read_to_string(format!("{UCD}/EastAsianWidth.txt")).expect("EastAsianWidth.txt");

    let ud = parse_unicode_data(&unicode_data);

    // --- XID identifier properties -----------------------------------------
    let xid_start = coalesce(collect_prop(&derived_core, "XID_Start"));
    let xid_continue = coalesce(collect_prop(&derived_core, "XID_Continue"));

    // --- Character width -----------------------------------------------------
    // Zero-width: nonspacing (Mn) / enclosing (Me) marks and format (Cf)
    // characters, plus the conjoining Hangul jungseong/jongseong jamo, minus
    // SOFT HYPHEN which occupies a column. See char_width docs for the contract.
    let mut zero: Vec<u32> = ud
        .iter()
        .filter(|(cp, r)| {
            matches!(r.gc.as_str(), "Mn" | "Me" | "Cf") && **cp != 0x00AD
        })
        .map(|(cp, _)| *cp)
        .collect();
    zero.extend(0x1160..=0x11FF); // conjoining Hangul jamo (medial + final)
    let zero_width = coalesce(zero);

    let wide = coalesce(collect_eaw_wide(&eaw));

    // --- Canonical combining class (non-zero only) --------------------------
    let ccc_ranges = coalesce_val(
        ud.iter()
            .filter(|(_, r)| r.ccc != 0)
            .map(|(cp, r)| (*cp, r.ccc))
            .collect(),
    );

    // --- Full decomposition (canonical and compatibility) -------------------
    let mut canon: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut compat: Vec<(u32, Vec<u32>)> = Vec::new();
    for (&cp, r) in &ud {
        if let Some(d) = &r.decomp {
            if !d.compat {
                canon.push((cp, full_canonical(cp, &ud)));
            }
            // Every character with any mapping decomposes fully for NFKD.
            compat.push((cp, full_compat(cp, &ud)));
        }
    }
    canon.sort_by_key(|(cp, _)| *cp);
    compat.sort_by_key(|(cp, _)| *cp);

    // --- Canonical composition pairs ----------------------------------------
    // (a, b) -> composite for every 2-char canonical decomposition whose
    // composite is not excluded from composition.
    let excluded = full_composition_exclusion(&derived_norm);
    let mut compose: Vec<(u64, u32)> = Vec::new();
    for (&cp, r) in &ud {
        if let Some(d) = &r.decomp {
            if !d.compat && d.to.len() == 2 && !excluded.contains(&cp) {
                let key = (u64::from(d.to[0]) << 32) | u64::from(d.to[1]);
                compose.push((key, cp));
            }
        }
    }
    compose.sort_by_key(|(k, _)| *k);

    // --- Quick-check tables --------------------------------------------------
    let nfc_qc = coalesce_val(collect_qc(&derived_norm, "NFC_QC"));
    let nfd_qc = coalesce_val(collect_qc(&derived_norm, "NFD_QC"));
    let nfkc_qc = coalesce_val(collect_qc(&derived_norm, "NFKC_QC"));
    let nfkd_qc = coalesce_val(collect_qc(&derived_norm, "NFKD_QC"));

    // --- Emit ----------------------------------------------------------------
    let mut o = String::new();
    let (maj, min, pat) = VERSION;
    o.push_str(
        "//! Generated Unicode lookup tables. DO NOT EDIT BY HAND.\n\
         //!\n\
         //! Produced by `dev/gen_tables.rs` from the Unicode Character Database.\n\
         //! Regenerate rather than editing; see that file for the procedure.\n\
         \n\
         // The normalization tables are unused when the `alloc` feature is off,\n\
         // so allow dead code rather than gating each static individually.\n\
         #![allow(dead_code)]\n\n",
    );
    writeln!(o, "/// Unicode version these tables were generated from.").ok();
    writeln!(o, "pub const UNICODE_VERSION: (u8, u8, u8) = ({maj}, {min}, {pat});\n").ok();

    emit_ranges(&mut o, "XID_START", &xid_start, "XID_Start code point ranges (sorted, inclusive).");
    emit_ranges(&mut o, "XID_CONTINUE", &xid_continue, "XID_Continue code point ranges (sorted, inclusive).");
    emit_ranges(&mut o, "ZERO_WIDTH", &zero_width, "Ranges rendered with zero display columns.");
    emit_ranges(&mut o, "WIDE", &wide, "East Asian Wide / Fullwidth ranges (two display columns).");
    emit_ranges_val(&mut o, "CCC", &ccc_ranges, "Non-zero canonical combining class, by range.");

    let canon_data = emit_decomp(&mut o, "CANON", &canon, "canonical");
    let compat_data = emit_decomp(&mut o, "COMPAT", &compat, "compatibility");
    let _ = (canon_data, compat_data);

    emit_compose(&mut o, &compose);

    emit_ranges_val(&mut o, "NFC_QC", &nfc_qc, "NFC quick-check (1 = No, 2 = Maybe).");
    emit_ranges_val(&mut o, "NFD_QC", &nfd_qc, "NFD quick-check (1 = No).");
    emit_ranges_val(&mut o, "NFKC_QC", &nfkc_qc, "NFKC quick-check (1 = No, 2 = Maybe).");
    emit_ranges_val(&mut o, "NFKD_QC", &nfkd_qc, "NFKD quick-check (1 = No).");

    fs::write(OUT, o).expect("write tables.rs");
    eprintln!(
        "wrote {OUT}: xid_start={} xid_continue={} zero={} wide={} ccc={} canon={} compat={} compose={}",
        xid_start.len(),
        xid_continue.len(),
        zero_width.len(),
        wide.len(),
        ccc_ranges.len(),
        canon.len(),
        compat.len(),
        compose.len(),
    );
}

struct Record {
    gc: String,
    ccc: u8,
    decomp: Option<Decomp>,
}

struct Decomp {
    compat: bool,
    to: Vec<u32>,
}

/// Parse `UnicodeData.txt`, expanding `First`/`Last` range rows.
fn parse_unicode_data(text: &str) -> BTreeMap<u32, Record> {
    let mut map = BTreeMap::new();
    let mut pending_first: Option<(u32, String, u8)> = None;
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(';').collect();
        let cp = u32::from_str_radix(f[0], 16).expect("cp");
        let name = f[1];
        let gc = f[2].to_string();
        let ccc: u8 = f[3].parse().expect("ccc");

        if name.ends_with(", First>") {
            pending_first = Some((cp, gc, ccc));
            continue;
        }
        if name.ends_with(", Last>") {
            let (start, gcf, cccf) = pending_first.take().expect("Last without First");
            for c in start..=cp {
                map.insert(c, Record { gc: gcf.clone(), ccc: cccf, decomp: None });
            }
            continue;
        }

        let decomp = parse_decomp(f[5]);
        map.insert(cp, Record { gc, ccc, decomp });
    }
    map
}

fn parse_decomp(field: &str) -> Option<Decomp> {
    if field.is_empty() {
        return None;
    }
    let mut compat = false;
    let mut to = Vec::new();
    for tok in field.split_whitespace() {
        if tok.starts_with('<') {
            compat = true; // compatibility tag, e.g. <compat>, <font>, <fraction>
            continue;
        }
        to.push(u32::from_str_radix(tok, 16).expect("decomp cp"));
    }
    Some(Decomp { compat, to })
}

/// Recursively apply canonical decomposition mappings only.
fn full_canonical(cp: u32, ud: &BTreeMap<u32, Record>) -> Vec<u32> {
    match ud.get(&cp).and_then(|r| r.decomp.as_ref()) {
        Some(d) if !d.compat => d.to.iter().flat_map(|&c| full_canonical(c, ud)).collect(),
        _ => vec![cp],
    }
}

/// Recursively apply the decomposition mapping (canonical or compatibility).
fn full_compat(cp: u32, ud: &BTreeMap<u32, Record>) -> Vec<u32> {
    match ud.get(&cp).and_then(|r| r.decomp.as_ref()) {
        Some(d) => d.to.iter().flat_map(|&c| full_compat(c, ud)).collect(),
        None => vec![cp],
    }
}

/// Collect a boolean property (e.g. `XID_Start`) as a flat list of code points.
fn collect_prop(text: &str, prop: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = strip_comment(line);
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(';').map(str::trim).collect();
        if parts.len() < 2 || parts[1] != prop {
            continue;
        }
        for c in parse_range(parts[0]) {
            out.push(c);
        }
    }
    out
}

/// Collect a quick-check property with its No(1)/Maybe(2) value.
fn collect_qc(text: &str, prop: &str) -> Vec<(u32, u8)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = strip_comment(line);
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(';').map(str::trim).collect();
        if parts.len() < 3 || parts[1] != prop {
            continue;
        }
        let val = match parts[2] {
            "N" => 1u8,
            "M" => 2u8,
            _ => continue,
        };
        for c in parse_range(parts[0]) {
            out.push((c, val));
        }
    }
    out
}

fn full_composition_exclusion(text: &str) -> std::collections::BTreeSet<u32> {
    let mut set = std::collections::BTreeSet::new();
    for line in text.lines() {
        let line = strip_comment(line);
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(';').map(str::trim).collect();
        if parts.len() < 2 || parts[1] != "Full_Composition_Exclusion" {
            continue;
        }
        for c in parse_range(parts[0]) {
            set.insert(c);
        }
    }
    set
}

fn collect_eaw_wide(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = strip_comment(line);
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(';').map(str::trim).collect();
        if parts.len() < 2 {
            continue;
        }
        if parts[1] == "W" || parts[1] == "F" {
            for c in parse_range(parts[0]) {
                out.push(c);
            }
        }
    }
    out
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

/// Parse `AAAA` or `AAAA..BBBB` into an iterator of code points.
fn parse_range(s: &str) -> impl Iterator<Item = u32> {
    let (a, b) = match s.split_once("..") {
        Some((a, b)) => (
            u32::from_str_radix(a.trim(), 16).expect("range start"),
            u32::from_str_radix(b.trim(), 16).expect("range end"),
        ),
        None => {
            let v = u32::from_str_radix(s.trim(), 16).expect("single cp");
            (v, v)
        }
    };
    a..=b
}

/// Merge a sorted-or-unsorted code point list into inclusive ranges.
fn coalesce(mut cps: Vec<u32>) -> Vec<(u32, u32)> {
    cps.sort_unstable();
    cps.dedup();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for c in cps {
        match out.last_mut() {
            Some(last) if c == last.1 + 1 => last.1 = c,
            _ => out.push((c, c)),
        }
    }
    out
}

/// Merge `(cp, value)` pairs into `(start, end, value)` ranges of equal value.
fn coalesce_val(mut pairs: Vec<(u32, u8)>) -> Vec<(u32, u32, u8)> {
    pairs.sort_unstable();
    pairs.dedup();
    let mut out: Vec<(u32, u32, u8)> = Vec::new();
    for (c, v) in pairs {
        match out.last_mut() {
            Some(last) if c == last.1 + 1 && v == last.2 => last.1 = c,
            _ => out.push((c, c, v)),
        }
    }
    out
}

fn emit_ranges(o: &mut String, name: &str, ranges: &[(u32, u32)], doc: &str) {
    writeln!(o, "/// {doc}").ok();
    writeln!(o, "pub static {name}: &[(u32, u32)] = &[").ok();
    for chunk in ranges.chunks(4) {
        o.push_str("    ");
        for (a, b) in chunk {
            write!(o, "(0x{a:04X}, 0x{b:04X}), ").ok();
        }
        o.push('\n');
    }
    o.push_str("];\n\n");
}

fn emit_ranges_val(o: &mut String, name: &str, ranges: &[(u32, u32, u8)], doc: &str) {
    writeln!(o, "/// {doc}").ok();
    writeln!(o, "pub static {name}: &[(u32, u32, u8)] = &[").ok();
    for chunk in ranges.chunks(4) {
        o.push_str("    ");
        for (a, b, v) in chunk {
            write!(o, "(0x{a:04X}, 0x{b:04X}, {v}), ").ok();
        }
        o.push('\n');
    }
    o.push_str("];\n\n");
}

/// Emit a decomposition table `NAME_DECOMP: &[(cp, off, len)]` plus its flat
/// `NAME_DATA: &[u32]` payload. Returns the payload length.
fn emit_decomp(o: &mut String, name: &str, entries: &[(u32, Vec<u32>)], kind: &str) -> usize {
    let mut data: Vec<u32> = Vec::new();
    let mut index: Vec<(u32, u32, u32)> = Vec::new();
    for (cp, to) in entries {
        let off = data.len() as u32;
        data.extend_from_slice(to);
        index.push((*cp, off, to.len() as u32));
    }

    writeln!(o, "/// Fully-expanded {kind} decomposition index: `(code point, offset, len)`.").ok();
    writeln!(o, "pub static {name}_DECOMP: &[(u32, u32, u32)] = &[").ok();
    for chunk in index.chunks(4) {
        o.push_str("    ");
        for (cp, off, len) in chunk {
            write!(o, "(0x{cp:04X}, {off}, {len}), ").ok();
        }
        o.push('\n');
    }
    o.push_str("];\n\n");

    writeln!(o, "/// Flat {kind} decomposition payload indexed by `{name}_DECOMP`.").ok();
    writeln!(o, "pub static {name}_DATA: &[u32] = &[").ok();
    for chunk in data.chunks(8) {
        o.push_str("    ");
        for c in chunk {
            write!(o, "0x{c:04X}, ").ok();
        }
        o.push('\n');
    }
    o.push_str("];\n\n");
    data.len()
}

fn emit_compose(o: &mut String, pairs: &[(u64, u32)]) {
    writeln!(o, "/// Canonical composition: packed `(first << 32 | second)` -> composite.").ok();
    writeln!(o, "pub static COMPOSE: &[(u64, u32)] = &[").ok();
    for chunk in pairs.chunks(3) {
        o.push_str("    ");
        for (k, v) in chunk {
            write!(o, "(0x{k:016X}, 0x{v:04X}), ").ok();
        }
        o.push('\n');
    }
    o.push_str("];\n");
}
