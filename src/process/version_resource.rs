//! Cross-platform reader for the Windows `VS_VERSIONINFO` resource of a PE
//! file (`fm.exe`, `game_plugin.dll`, ...).
//!
//! Parsing is done with `pelite` on a read-only memory map of the file, so it
//! works identically on Windows and on Linux (Proton installs) without any
//! Win32 API and without copying the whole module into a heap buffer. Every
//! failure — unreadable file, non-PE data, truncated headers, missing or
//! corrupt version resource — is reported as an `Err(String)`; nothing here
//! panics on malformed input and nothing here touches process memory.

use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};

use pelite::resources::version_info::Language;
use pelite::{FileMap, PeFile};

/// String-table values from a module's version resource.
///
/// Values are collected from every string table (translation) present, in
/// the order declared by `\VarFileInfo\Translation` followed by any other
/// tables in ascending language/code-page order. Values are trimmed, empty
/// values are skipped and duplicates are removed (first occurrence wins).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct VersionMetadata {
    pub product_names: Vec<String>,
    pub product_versions: Vec<String>,
    pub file_versions: Vec<String>,
}

/// Reads the version resource of the PE file at `path`.
///
/// Returns an error when the file cannot be mapped, is not a valid PE image,
/// has no version resource, or the resource contains none of
/// `ProductName`, `ProductVersion` and `FileVersion`.
pub(super) fn read(path: &str) -> Result<VersionMetadata, String> {
    let map = FileMap::open(path).map_err(|e| format!("{path}: cannot map file: {e}"))?;
    // pelite reports malformed data through `Result`; the unwind guard is
    // defence in depth so a parser bug on hostile input surfaces as an error
    // instead of taking down the worker thread.
    panic::catch_unwind(AssertUnwindSafe(|| parse(map.as_ref())))
        .unwrap_or_else(|_| Err("version resource parser panicked".to_string()))
        .map_err(|e| format!("{path}: {e}"))
}

fn parse(bytes: &[u8]) -> Result<VersionMetadata, String> {
    let pe = PeFile::from_bytes(bytes).map_err(|e| format!("not a valid PE file: {e}"))?;
    let resources = pe
        .resources()
        .map_err(|e| format!("no resource directory: {e}"))?;
    let version_info = resources
        .version_info()
        .map_err(|e| format!("no version resource: {e}"))?;
    let file_info = version_info.file_info();

    let mut languages: Vec<Language> = Vec::new();
    for lang in file_info.langs {
        if file_info.strings.contains_key(lang) && !languages.contains(lang) {
            languages.push(*lang);
        }
    }
    let mut rest: Vec<Language> = file_info
        .strings
        .keys()
        .filter(|lang| !languages.contains(lang))
        .copied()
        .collect();
    rest.sort_by_key(|lang| (lang.lang_id, lang.charset_id));
    languages.extend(rest);

    let mut metadata = VersionMetadata::default();
    for lang in &languages {
        let Some(table) = file_info.strings.get(lang) else {
            continue;
        };
        // Sort keys so lookups are deterministic if a table repeats a key in
        // different letter case (VerQueryValue matches case-insensitively).
        let mut entries: Vec<(&String, &String)> = table.iter().collect();
        entries.sort();
        for (key, value) in entries {
            let target = if key.eq_ignore_ascii_case("ProductName") {
                &mut metadata.product_names
            } else if key.eq_ignore_ascii_case("ProductVersion") {
                &mut metadata.product_versions
            } else if key.eq_ignore_ascii_case("FileVersion") {
                &mut metadata.file_versions
            } else {
                continue;
            };
            let value = value.trim_matches(|c: char| c.is_whitespace() || c == '\0');
            if !value.is_empty() {
                target.push(value.to_string());
            }
        }
    }
    dedup(&mut metadata.product_names);
    dedup(&mut metadata.product_versions);
    dedup(&mut metadata.file_versions);

    if metadata.product_names.is_empty()
        && metadata.product_versions.is_empty()
        && metadata.file_versions.is_empty()
    {
        return Err(
            "version resource has no ProductName, ProductVersion or FileVersion string".to_string(),
        );
    }
    Ok(metadata)
}

fn dedup(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|v| seen.insert(v.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ---- temp file helper -------------------------------------------------

    struct TempFile(PathBuf);
    impl TempFile {
        fn new(bytes: &[u8]) -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "fm-editor-version-resource-{}-{}.bin",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(&path, bytes).expect("write temp file");
            TempFile(path)
        }
        fn path(&self) -> &str {
            self.0.to_str().expect("utf-8 temp path")
        }
    }
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    // ---- VS_VERSIONINFO builder ------------------------------------------

    fn pad4(buf: &mut Vec<u8>) {
        while buf.len() % 4 != 0 {
            buf.push(0);
        }
    }

    fn utf16z(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    fn node(key: &str, value: &[u8], value_len: u16, wtype: u16, children: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0u8; 6];
        out.extend(utf16z(key));
        pad4(&mut out);
        out.extend_from_slice(value);
        for child in children {
            pad4(&mut out);
            out.extend_from_slice(child);
        }
        let len = out.len() as u16;
        out[0..2].copy_from_slice(&len.to_le_bytes());
        out[2..4].copy_from_slice(&value_len.to_le_bytes());
        out[4..6].copy_from_slice(&wtype.to_le_bytes());
        out
    }

    fn string(key: &str, value: &str) -> Vec<u8> {
        let v = utf16z(value);
        node(key, &v, (v.len() / 2) as u16, 1, &[])
    }

    fn version_info(tables: &[(&str, &[(&str, &str)])], translations: &[(u16, u16)]) -> Vec<u8> {
        let mut fixed = Vec::new();
        fixed.extend(0xFEEF_04BDu32.to_le_bytes());
        fixed.extend(0x0001_0000u32.to_le_bytes());
        fixed.resize(52, 0);
        let tables: Vec<Vec<u8>> = tables
            .iter()
            .map(|(lang, strings)| {
                let children: Vec<Vec<u8>> = strings.iter().map(|(k, v)| string(k, v)).collect();
                node(lang, &[], 0, 1, &children)
            })
            .collect();
        let string_file_info = node("StringFileInfo", &[], 0, 1, &tables);
        let trans: Vec<u8> = translations
            .iter()
            .flat_map(|(l, c)| l.to_le_bytes().into_iter().chain(c.to_le_bytes()))
            .collect();
        let var = node("Translation", &trans, trans.len() as u16, 0, &[]);
        let var_file_info = node("VarFileInfo", &[], 0, 1, &[var]);
        node(
            "VS_VERSION_INFO",
            &fixed,
            52,
            0,
            &[string_file_info, var_file_info],
        )
    }

    // ---- minimal PE32+ builder ------------------------------------------

    const SECTION_RVA: u32 = 0x1000;
    const HEADERS_SIZE: usize = 0x200;

    /// Resource directory: RT_VERSION / ID 1 / LANG 0x409 -> `data`.
    fn resource_section(data: &[u8]) -> Vec<u8> {
        fn dir(entry_name: u32, entry_target: u32) -> Vec<u8> {
            let mut d = vec![0u8; 12];
            d.extend(0u16.to_le_bytes()); // named entries
            d.extend(1u16.to_le_bytes()); // id entries
            d.extend(entry_name.to_le_bytes());
            d.extend(entry_target.to_le_bytes());
            d
        }
        let mut s = Vec::new();
        s.extend(dir(16, 0x8000_0000 | 24)); // RT_VERSION -> dir at 24
        s.extend(dir(1, 0x8000_0000 | 48)); // ID 1 -> dir at 48
        s.extend(dir(0x409, 72)); // LANG -> data entry at 72
        s.extend((SECTION_RVA + 96).to_le_bytes()); // data RVA
        s.extend((data.len() as u32).to_le_bytes());
        s.extend([0u8; 8]); // code page, reserved
        s.resize(96, 0);
        s.extend_from_slice(data);
        s
    }

    fn build_pe(rsrc: Option<&[u8]>) -> Vec<u8> {
        let section = rsrc.map(resource_section).unwrap_or_else(|| vec![0u8; 16]);
        let raw_size = section.len().div_ceil(0x200) * 0x200;
        let mut pe = vec![0u8; HEADERS_SIZE];
        let put16 =
            |pe: &mut Vec<u8>, at: usize, v: u16| pe[at..at + 2].copy_from_slice(&v.to_le_bytes());
        let put32 =
            |pe: &mut Vec<u8>, at: usize, v: u32| pe[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let put64 =
            |pe: &mut Vec<u8>, at: usize, v: u64| pe[at..at + 8].copy_from_slice(&v.to_le_bytes());

        pe[0..2].copy_from_slice(b"MZ");
        put32(&mut pe, 0x3C, 0x40);
        pe[0x40..0x44].copy_from_slice(b"PE\0\0");
        let coff = 0x44;
        put16(&mut pe, coff, 0x8664);
        put16(&mut pe, coff + 2, 1);
        put16(&mut pe, coff + 16, 0xF0);
        put16(&mut pe, coff + 18, 0x2022);
        let opt = 0x58;
        put16(&mut pe, opt, 0x20B);
        put64(&mut pe, opt + 24, 0x1_4000_0000);
        put32(&mut pe, opt + 32, 0x1000); // section alignment
        put32(&mut pe, opt + 36, 0x200); // file alignment
        put16(&mut pe, opt + 40, 6);
        put16(&mut pe, opt + 48, 6);
        put32(&mut pe, opt + 56, SECTION_RVA + 0x1000); // size of image
        put32(&mut pe, opt + 60, HEADERS_SIZE as u32);
        put16(&mut pe, opt + 68, 2);
        put32(&mut pe, opt + 108, 16);
        if rsrc.is_some() {
            put32(&mut pe, opt + 112 + 2 * 8, SECTION_RVA);
            put32(&mut pe, opt + 112 + 2 * 8 + 4, section.len() as u32);
        }
        let sh = opt + 0xF0;
        pe[sh..sh + 5].copy_from_slice(b".rsrc");
        put32(&mut pe, sh + 8, section.len() as u32);
        put32(&mut pe, sh + 12, SECTION_RVA);
        put32(&mut pe, sh + 16, raw_size as u32);
        put32(&mut pe, sh + 20, HEADERS_SIZE as u32);
        put32(&mut pe, sh + 36, 0x4000_0040);

        pe.extend_from_slice(&section);
        pe.resize(HEADERS_SIZE + raw_size, 0);
        pe
    }

    fn fm_like_version_info() -> Vec<u8> {
        version_info(
            &[
                (
                    "040904b0",
                    &[
                        ("ProductName", "Football Manager 27"),
                        ("ProductVersion", "27.0.0+2493426"),
                        ("FileVersion", "27.0.0.0"),
                        ("CompanyName", "Sports Interactive"),
                    ],
                ),
                (
                    "080904b0",
                    &[
                        ("ProductName", "Football Manager 27"),
                        ("ProductVersion", "27.0.1+2493999"),
                        ("FileVersion", "  "),
                    ],
                ),
            ],
            &[(0x0409, 0x04B0)],
        )
    }

    // ---- tests -----------------------------------------------------------

    #[test]
    fn reads_all_string_tables_and_dedups() {
        let file = TempFile::new(&build_pe(Some(&fm_like_version_info())));
        let meta = read(file.path()).expect("valid synthetic PE");
        assert_eq!(meta.product_names, vec!["Football Manager 27"]);
        assert_eq!(
            meta.product_versions,
            vec!["27.0.0+2493426", "27.0.1+2493999"]
        );
        assert_eq!(meta.file_versions, vec!["27.0.0.0"]);
    }

    #[test]
    fn missing_file_is_error() {
        let path = std::env::temp_dir().join(format!(
            "fm-editor-version-resource-missing-{}.exe",
            std::process::id()
        ));
        let err = read(path.to_str().unwrap()).unwrap_err();
        assert!(err.contains("cannot map file"), "{err}");
    }

    #[test]
    fn empty_file_is_error() {
        let file = TempFile::new(&[]);
        assert!(read(file.path()).is_err());
    }

    #[test]
    fn non_pe_is_error() {
        let file = TempFile::new(
            b"this is definitely not a portable executable\n"
                .repeat(64)
                .as_slice(),
        );
        let err = read(file.path()).unwrap_err();
        assert!(err.contains("not a valid PE"), "{err}");
    }

    #[test]
    fn mz_without_pe_header_is_error() {
        let mut bytes = vec![0u8; 0x400];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x7FFF_FFF0u32.to_le_bytes());
        let file = TempFile::new(&bytes);
        assert!(read(file.path()).is_err());
    }

    #[test]
    fn truncated_pe_is_error() {
        let full = build_pe(Some(&fm_like_version_info()));
        for cut in [2, 0x40, 0x60, 0x150, HEADERS_SIZE, HEADERS_SIZE + 40] {
            let file = TempFile::new(&full[..cut]);
            assert!(
                read(file.path()).is_err(),
                "truncated at {cut:#x} should fail"
            );
        }
    }

    #[test]
    fn pe_without_resources_is_error() {
        let file = TempFile::new(&build_pe(None));
        let err = read(file.path()).unwrap_err();
        assert!(err.contains("resource"), "{err}");
    }

    #[test]
    fn corrupt_version_blob_is_error() {
        let file = TempFile::new(&build_pe(Some(&[0xFFu8; 64])));
        assert!(read(file.path()).is_err());
    }

    #[test]
    fn version_resource_without_wanted_strings_is_error() {
        let blob = version_info(
            &[("040904b0", &[("CompanyName", "X")])],
            &[(0x0409, 0x04B0)],
        );
        let file = TempFile::new(&build_pe(Some(&blob)));
        let err = read(file.path()).unwrap_err();
        assert!(err.contains("no ProductName"), "{err}");
    }

    /// Manual check against an installed module:
    /// `FM_VERSION_TEST_MODULE=/path/to/fm.exe cargo test version_resource -- --ignored --nocapture`
    #[test]
    #[ignore = "requires FM_VERSION_TEST_MODULE pointing at an installed PE module"]
    fn reads_module_from_env() {
        let path = std::env::var("FM_VERSION_TEST_MODULE")
            .expect("set FM_VERSION_TEST_MODULE to a PE file path");
        let meta = read(&path).expect("read version resource");
        println!("{path}: {meta:#?}");
        assert!(!meta.product_names.is_empty() || !meta.product_versions.is_empty());
    }
}
