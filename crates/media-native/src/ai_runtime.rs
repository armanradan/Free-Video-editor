//! Read-only optional runtime discovery. Never loads a DLL, downloads assets,
//! installs a toolchain/driver, accepts SDK terms or grants product availability.
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default)]
pub struct RuntimePaths {
    pub onnx_directory: Option<PathBuf>,
    pub rtx_sdk: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct Component {
    pub path: Option<PathBuf>,
    pub status: &'static str,
    pub reason: String,
    pub product_version: Option<[u16; 4]>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeReport {
    pub onnx: Vec<Component>,
    pub rtx: Vec<Component>,
    pub inference_available: bool,
    pub sdk_terms_accepted_by_this_operation: bool,
}

impl RuntimePaths {
    /// Explicit paths take precedence; environment discovery never guesses a
    /// developer's SDK folder or searches executable/library system directories.
    pub fn from_environment() -> Self {
        Self {
            onnx_directory: std::env::var_os("DIAXUS_ORT_DIRECTORY")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            rtx_sdk: std::env::var_os("DIAXUS_RTX_SDK")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }

    pub fn inspect(&self) -> RuntimeReport {
        let onnx = if let Some(root) = &self.onnx_directory {
            vec![
                inspect(
                    &root.join("onnxruntime.dll"),
                    Some([1, 23]),
                    Some([1, 23, 25, 924]),
                ),
                inspect(&root.join("DirectML.dll"), None, Some([1, 15, 4, 0])),
            ]
        } else {
            vec![missing(
                None,
                "Set an explicit ONNX directory or DIAXUS_ORT_DIRECTORY; no runtime is installed automatically",
            )]
        };
        let rtx = if let Some(root) = &self.rtx_sdk {
            let mut components = vec![inspect(
                &root.join("bin/Windows/x64/rel/nvngx_vsr.dll"),
                None,
                Some([1, 6, 0, 0]),
            )];
            for relative in [
                "include/nvsdk_ngx.h",
                "include/nvsdk_ngx_helpers_vsr.h",
                "NVIDIA_RTX_Video_SDK_License.pdf",
            ] {
                let path = root.join(relative);
                components.push(if path.is_file() {Component {path:Some(path),status:"present-unvalidated",
                    reason:"Assessed SDK layout only; package identity, API/driver/toolchain and terms still require their separate gates".into(),product_version:None}}
                    else {missing(Some(path),"Required assessed RTX SDK component is missing")});
            }
            components
        } else {
            vec![missing(
                None,
                "Set an explicit RTX SDK root or DIAXUS_RTX_SDK; proprietary SDK assets are never bundled or installed automatically",
            )]
        };
        RuntimeReport {
            onnx,
            rtx,
            inference_available: false,
            sdk_terms_accepted_by_this_operation: false,
        }
    }
}

fn missing(path: Option<PathBuf>, reason: &str) -> Component {
    Component {
        path,
        status: "missing",
        reason: reason.into(),
        product_version: None,
    }
}

fn inspect(path: &Path, prefix: Option<[u16; 2]>, exact: Option<[u16; 4]>) -> Component {
    if !path.is_file() {
        return missing(
            Some(path.to_owned()),
            "Runtime component is missing; explicitly provide the matching x64 runtime package",
        );
    }
    let result = (|| {
        let length = fs::metadata(path).map_err(|e| e.to_string())?.len();
        if length > 64 * 1024 * 1024 {
            return Err("Runtime metadata inspection exceeds the 64 MiB file bound".into());
        }
        let mut data = Vec::with_capacity(length as usize);
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 64 * 1024 * 1024 {
            return Err("Runtime grew beyond metadata inspection bound".into());
        }
        parse_pe(&data)
    })();
    match result {
        Err(reason) => Component {
            path: Some(path.to_owned()),
            status: "incompatible",
            reason,
            product_version: None,
        },
        Ok(version) => {
            let compatible =
                prefix.is_none_or(|p| version[..2] == p) && exact.is_none_or(|v| version == v);
            Component {
                path: Some(path.to_owned()),
                status: if compatible {
                    "present-unvalidated"
                } else {
                    "incompatible"
                },
                reason: if compatible {
                    "x64 DLL and assessed PE product version found; C API/provider/device/model execution and cleanup remain separate gates. No library was loaded".into()
                } else {
                    format!(
                        "Wrong runtime product version {version:?}; expected prefix {prefix:?}, exact {exact:?}. Explicitly select the assessed package"
                    )
                },
                product_version: Some(version),
            }
        }
    }
}

fn parse_pe(bytes: &[u8]) -> Result<[u16; 4], String> {
    let bad =
        || "Malformed or unsupported PE metadata; provide the original x64 runtime DLL".to_owned();
    let word = |at: usize| -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            bytes
                .get(at..at.checked_add(2).ok_or_else(bad)?)
                .ok_or_else(bad)?
                .try_into()
                .map_err(|_| bad())?,
        ))
    };
    let dword = |at: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            bytes
                .get(at..at.checked_add(4).ok_or_else(bad)?)
                .ok_or_else(bad)?
                .try_into()
                .map_err(|_| bad())?,
        ))
    };
    if bytes.get(..2) != Some(b"MZ") {
        return Err(bad());
    }
    let pe = dword(0x3c)? as usize;
    if pe > 1024 * 1024 || bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(bad());
    }
    if word(pe + 4)? != 0x8664 {
        return Err("Wrong runtime architecture: an AMD64/x64 DLL is required".into());
    }
    let sections = word(pe + 6)? as usize;
    let optional = pe + 24;
    let optional_size = word(pe + 20)? as usize;
    if sections == 0
        || sections > 96
        || optional_size < 136
        || word(optional)? != 0x20b
        || word(pe + 22)? & 0x2000 == 0
    {
        return Err(bad());
    }
    let section_start = optional + optional_size;
    let resolve = |rva: u32| -> Result<usize, String> {
        for index in 0..sections {
            let s = section_start + index * 40;
            let start = dword(s + 12)?;
            let size = dword(s + 16)?;
            if let Some(offset) = rva.checked_sub(start).filter(|offset| *offset < size) {
                return (dword(s + 20)? as usize)
                    .checked_add(offset as usize)
                    .filter(|at| *at < bytes.len())
                    .ok_or_else(bad);
            }
        }
        Err(bad())
    };
    let resource_rva = dword(optional + 128)?;
    let resource_size = dword(optional + 132)? as usize;
    if resource_rva == 0 || resource_size < 24 {
        return Err(
            "Runtime product-version resource is missing; version cannot be verified".into(),
        );
    }
    let base = resolve(resource_rva)?;
    let entry = |offset: usize, id: Option<u32>| -> Result<u32, String> {
        if offset.checked_add(16).is_none_or(|n| n > resource_size) {
            return Err(bad());
        }
        let count = usize::from(word(base + offset + 12)?) + usize::from(word(base + offset + 14)?);
        if count == 0 || count > 512 {
            return Err(bad());
        }
        for index in 0..count {
            let pos = offset + 16 + index * 8;
            if pos + 8 > resource_size {
                return Err(bad());
            }
            if id.is_none_or(|wanted| dword(base + pos).is_ok_and(|actual| actual == wanted)) {
                return dword(base + pos + 4);
            }
        }
        Err("Runtime product-version resource is missing".into())
    };
    let resource = entry(0, Some(16))?;
    if resource & 0x80000000 == 0 {
        return Err(bad());
    }
    let name = entry((resource & 0x7fffffff) as usize, None)?;
    if name & 0x80000000 == 0 {
        return Err(bad());
    }
    let language = entry((name & 0x7fffffff) as usize, None)?;
    if language & 0x80000000 != 0 || language as usize + 16 > resource_size {
        return Err(bad());
    }
    let data = resolve(dword(base + language as usize)?)?;
    let size = dword(base + language as usize + 4)? as usize;
    if !(60..=65536).contains(&size) || data.checked_add(size).is_none_or(|end| end > bytes.len()) {
        return Err(bad());
    }
    let key = "VS_VERSION_INFO\0".encode_utf16().collect::<Vec<_>>();
    if word(data)? as usize > size || word(data + 2)? < 52 {
        return Err(bad());
    }
    for (index, value) in key.iter().enumerate() {
        if word(data + 6 + index * 2)? != *value {
            return Err(bad());
        }
    }
    let fixed = (data + 6 + key.len() * 2 + 3) & !3;
    if fixed + 52 > data + size || dword(fixed)? != 0xfeef04bd {
        return Err(bad());
    }
    let ms = dword(fixed + 16)?;
    let ls = dword(fixed + 20)?;
    Ok([(ms >> 16) as u16, ms as u16, (ls >> 16) as u16, ls as u16])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binary(version: [u16; 4]) -> Vec<u8> {
        fn w(b: &mut [u8], at: usize, v: u16) {
            b[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
        fn d(b: &mut [u8], at: usize, v: u32) {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        let mut b = vec![0u8; 1024];
        b[..2].copy_from_slice(b"MZ");
        d(&mut b, 0x3c, 64);
        b[64..68].copy_from_slice(b"PE\0\0");
        w(&mut b, 68, 0x8664);
        w(&mut b, 70, 1);
        w(&mut b, 84, 240);
        w(&mut b, 86, 0x2000);
        w(&mut b, 88, 0x20b);
        d(&mut b, 88 + 128, 0x1000);
        d(&mut b, 88 + 132, 384);
        d(&mut b, 328 + 12, 0x1000);
        d(&mut b, 328 + 16, 512);
        d(&mut b, 328 + 20, 512);
        for offset in [0, 32, 64] {
            w(&mut b, 512 + offset + 14, 1);
        }
        d(&mut b, 528, 16);
        d(&mut b, 532, 0x80000020);
        d(&mut b, 560, 1);
        d(&mut b, 564, 0x80000040);
        d(&mut b, 592, 1033);
        d(&mut b, 596, 96);
        d(&mut b, 608, 0x1080);
        d(&mut b, 612, 92);
        w(&mut b, 640, 92);
        w(&mut b, 642, 52);
        for (i, c) in "VS_VERSION_INFO\0".encode_utf16().enumerate() {
            w(&mut b, 646 + i * 2, c);
        }
        d(&mut b, 680, 0xfeef04bd);
        d(&mut b, 684, 0x10000);
        d(
            &mut b,
            696,
            (u32::from(version[0]) << 16) | u32::from(version[1]),
        );
        d(
            &mut b,
            700,
            (u32::from(version[2]) << 16) | u32::from(version[3]),
        );
        b
    }

    #[test]
    fn version_resource_bounds_and_explicit_wrong_version_recovery() {
        let version = [1, 23, 25, 924];
        assert_eq!(parse_pe(&binary(version)).unwrap(), version);
        let mut corrupt = binary(version);
        corrupt[608..612].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_pe(&corrupt).is_err());
        let file = std::env::temp_dir().join(format!(
            "diaxus-ai-runtime-metadata-{}-{}.dll",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut owned = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .unwrap();
        use std::io::Write;
        owned.write_all(&binary([1, 99, 0, 0])).unwrap();
        drop(owned);
        assert_eq!(inspect(&file, Some([1, 23]), None).status, "incompatible");
        fs::write(&file, binary(version)).unwrap();
        let recovered = inspect(&file, Some([1, 23]), None);
        assert_eq!(recovered.status, "present-unvalidated");
        assert_eq!(recovered.product_version, Some(version));
        assert_eq!(
            inspect(&file, None, Some([1, 6, 0, 0])).status,
            "incompatible"
        );
        fs::remove_file(&file).unwrap();
        assert_eq!(inspect(&file, Some([1, 23]), None).status, "missing");
    }
    #[test]
    fn malformed_and_wrong_architecture_metadata_never_loads() {
        for bytes in [vec![], vec![0; 128], b"MZ".to_vec()] {
            assert!(parse_pe(&bytes).is_err());
        }
        let mut bytes = vec![0; 256];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
        assert!(parse_pe(&bytes).unwrap_err().contains("architecture"));
        bytes[0x3c..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_pe(&bytes).is_err());
    }
    #[test]
    fn absent_optional_paths_are_unavailable_without_creating_files() {
        let report = RuntimePaths::default().inspect();
        assert!(!report.inference_available && !report.sdk_terms_accepted_by_this_operation);
        assert_eq!(report.onnx[0].status, "missing");
        assert_eq!(report.rtx[0].status, "missing");
    }
}
