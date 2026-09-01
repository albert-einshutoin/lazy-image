use std::{env, fs, path::PathBuf, process::Command};

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut message = bytes.to_vec();
    let bit_len = (message.len() as u64) * 8;
    message.push(0x80);
    while (message.len() + 8) % 64 != 0 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in message.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (index, word) in schedule[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes([
                chunk[index * 4],
                chunk[index * 4 + 1],
                chunk[index * 4 + 2],
                chunk[index * 4 + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let mut working = state;
        for (index, constant) in SHA256_K.iter().enumerate() {
            let s1 = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let choice = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
            let temp1 = working[7]
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(*constant)
                .wrapping_add(schedule[index]);
            let s0 = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let temp2 = s0.wrapping_add(majority);
            working = [
                temp1.wrapping_add(temp2),
                working[0],
                working[1],
                working[2],
                working[3].wrapping_add(temp1),
                working[4],
                working[5],
                working[6],
            ];
        }
        for index in 0..8 {
            state[index] = state[index].wrapping_add(working[index]);
        }
    }

    let mut digest = [0u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

const EXPLICIT_ENV_KEYS: &[&str] = &[
    "AR",
    "CC",
    "CFLAGS",
    "CPPFLAGS",
    "CXX",
    "CXXFLAGS",
    "LIBAVIF_CFLAGS",
    "LIBAVIF_LIBS",
    "MOZJPEG_CFLAGS",
    "MOZJPEG_LIBS",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "TARGET_AR",
    "TARGET_CC",
    "TARGET_CFLAGS",
    "TARGET_CXX",
    "TARGET_CXXFLAGS",
    "WEBP_CFLAGS",
    "WEBP_LIBS",
    "PKG_CONFIG_PATH",
    "PKG_CONFIG_LIBDIR",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
];

const TARGET_TOOLCHAIN_ENV_KEYS: &[&str] = &[
    "AR",
    "CC",
    "CFLAGS",
    "CPPFLAGS",
    "CXX",
    "CXXFLAGS",
    "PKG_CONFIG_PATH",
    "PKG_CONFIG_LIBDIR",
];

const KNOWN_FEATURES: &[&str] = &["NAPI", "JEMALLOC", "AVIF", "FUZZING", "STRESS", "COW_DEBUG"];

const KNOWN_CFG_KEYS: &[&str] = &[
    "CARGO_CFG_DEBUG_ASSERTIONS",
    "CARGO_CFG_OVERFLOW_CHECKS",
    "CARGO_CFG_PANIC",
    "CARGO_CFG_PROC_MACRO",
    "CARGO_CFG_TARGET_ABI",
    "CARGO_CFG_TARGET_ARCH",
    "CARGO_CFG_TARGET_ENDIAN",
    "CARGO_CFG_TARGET_ENV",
    "CARGO_CFG_TARGET_FAMILY",
    "CARGO_CFG_TARGET_FEATURE",
    "CARGO_CFG_TARGET_HAS_ATOMIC",
    "CARGO_CFG_TARGET_OS",
    "CARGO_CFG_TARGET_POINTER_WIDTH",
    "CARGO_CFG_UNIX",
    "CARGO_CFG_WINDOWS",
];

const PROFILE_FIELDS: &[&str] = &[
    "CODEGEN_UNITS",
    "DEBUG",
    "DEBUG_ASSERTIONS",
    "INCREMENTAL",
    "LTO",
    "OPT_LEVEL",
    "OVERFLOW_CHECKS",
    "PANIC",
    "RPATH",
    "STRIP",
];

const TARGET_PROFILE_FIELDS: &[&str] = &[
    "AR",
    "CC",
    "CFLAGS",
    "CRATE_TYPE",
    "CXX",
    "CXXFLAGS",
    "LINKER",
    "RUSTFLAGS",
    "RUNNER",
];

const KNOWN_DEP_KEYS: &[&str] = &[
    "DEP_AOM_INCLUDE",
    "DEP_AOM_PKGCONFIG",
    "DEP_DAV1D_INCLUDE",
    "DEP_DAV1D_PKGCONFIG",
    "DEP_DAV1D_STATICLIB",
    "DEP_JPEG_ABI",
    "DEP_JPEG_INCLUDE",
];

fn output_environment_key(key: &str, target: &str) -> bool {
    if key.starts_with("CARGO_FEATURE_")
        || key.starts_with("CARGO_CFG_")
        || key.starts_with("CARGO_PROFILE_")
        || key.starts_with("CARGO_TARGET_")
        || key.starts_with("DEP_")
        || EXPLICIT_ENV_KEYS.contains(&key)
    {
        return true;
    }
    let normalized_target = target.replace('-', "_");
    let variants = [target, normalized_target.as_str()];
    let toolchain_names = [
        "AR",
        "CC",
        "CFLAGS",
        "CPPFLAGS",
        "CXX",
        "CXXFLAGS",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_LIBDIR",
    ];
    variants.iter().any(|variant| {
        toolchain_names
            .iter()
            .any(|name| key == format!("{name}_{variant}") || key == format!("{variant}_{name}"))
    })
}

fn selected_environment(target: &str) -> Vec<(String, String)> {
    let mut values = env::vars()
        .filter(|(key, _)| output_environment_key(key, target))
        .collect::<Vec<_>>();
    values.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    values
}

fn monitored_environment_keys(target: &str, profile: &str) -> Vec<String> {
    let mut keys = EXPLICIT_ENV_KEYS
        .iter()
        .map(|key| (*key).to_string())
        .chain(
            KNOWN_FEATURES
                .iter()
                .map(|feature| format!("CARGO_FEATURE_{feature}")),
        )
        .chain(KNOWN_CFG_KEYS.iter().map(|key| (*key).to_string()))
        .chain(KNOWN_DEP_KEYS.iter().map(|key| (*key).to_string()))
        .collect::<Vec<_>>();

    let profile_name = profile.to_ascii_uppercase().replace('-', "_");
    keys.extend(
        PROFILE_FIELDS
            .iter()
            .map(|field| format!("CARGO_PROFILE_{profile_name}_{field}")),
    );

    let normalized_target = target.replace('-', "_");
    for variant in [target, normalized_target.as_str()] {
        keys.extend(
            TARGET_PROFILE_FIELDS
                .iter()
                .map(|field| format!("CARGO_TARGET_{}_{}", variant.to_ascii_uppercase(), field)),
        );
        keys.extend(
            TARGET_TOOLCHAIN_ENV_KEYS
                .iter()
                .flat_map(|name| [format!("{name}_{variant}"), format!("{variant}_{name}")]),
        );
    }

    // Prefix-based Cargo metadata is intentionally retained in the digest. Add
    // the keys visible in this invocation and the static keys above to cover
    // unset-to-set changes on the next incremental build.
    keys.extend(
        env::vars()
            .filter(|(key, _)| output_environment_key(key, target))
            .map(|(key, _)| key),
    );
    keys.sort_unstable();
    keys.dedup();
    keys
}

fn compiler_config_digest(
    cargo_toml: &[u8],
    cargo_lock: &[u8],
    target: &str,
    profile: &str,
    rustc_identity: &[u8],
    environment: &[(String, String)],
) -> String {
    let mut identity = Vec::new();
    for (label, bytes) in [
        ("cargo.toml", cargo_toml),
        ("cargo.lock", cargo_lock),
        ("target", target.as_bytes()),
        ("profile", profile.as_bytes()),
        ("rustc", rustc_identity),
    ] {
        identity.extend_from_slice(label.as_bytes());
        identity.push(0);
        identity.extend_from_slice(bytes);
        identity.push(0);
    }
    for (key, value) in environment {
        identity.extend_from_slice(key.as_bytes());
        identity.push(0);
        identity.extend_from_slice(value.as_bytes());
        identity.push(0);
    }
    hex(&sha256(&identity))
}

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let cargo_toml = fs::read(manifest_dir.join("Cargo.toml")).expect("Cargo.toml");
    let cargo_lock = fs::read(manifest_dir.join("Cargo.lock")).expect("Cargo.lock");
    let target = env::var("TARGET").expect("target");
    let profile = env::var("PROFILE").expect("profile");
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let rustc_output = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("rustc -vV must execute");
    assert!(rustc_output.status.success(), "rustc -vV failed");
    assert!(
        !rustc_output.stdout.is_empty(),
        "rustc -vV returned no identity"
    );
    let rustc_identity = rustc_output.stdout;
    let environment = selected_environment(&target);
    for key in monitored_environment_keys(&target, &profile) {
        println!("cargo:rerun-if-env-changed={key}");
    }

    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!(
        "cargo:rustc-env=LAZY_IMAGE_COMPILER_CONFIG={}",
        compiler_config_digest(
            &cargo_toml,
            &cargo_lock,
            &target,
            &profile,
            &rustc_identity,
            &environment,
        )
    );
    println!("cargo:rustc-env=LAZY_IMAGE_COMPILER_TARGET={target}");
    println!("cargo:rustc-env=LAZY_IMAGE_COMPILER_PROFILE={profile}");
}

#[cfg(test)]
mod tests {
    use super::{
        compiler_config_digest, hex, monitored_environment_keys, output_environment_key, sha256,
    };

    #[test]
    fn sha256_matches_standard_vector() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }

    #[test]
    fn compiler_digest_includes_feature_configuration() {
        let base = vec![("CARGO_FEATURE_AVIF".to_string(), "1".to_string())];
        let mut changed = base.clone();
        changed.push(("CARGO_FEATURE_STRESS".to_string(), "1".to_string()));
        assert_ne!(
            compiler_config_digest(b"manifest", b"lock", "target", "release", b"rustc", &base),
            compiler_config_digest(
                b"manifest",
                b"lock",
                "target",
                "release",
                b"rustc",
                &changed
            ),
        );
    }

    #[test]
    fn target_qualified_toolchain_keys_are_identity_inputs() {
        assert!(output_environment_key(
            "CC_aarch64_apple_darwin",
            "aarch64-apple-darwin"
        ));
        assert!(output_environment_key(
            "aarch64-apple-darwin_CFLAGS",
            "aarch64-apple-darwin"
        ));
    }

    #[test]
    fn monitoring_includes_unset_toolchain_and_profile_keys() {
        let keys = monitored_environment_keys("aarch64-apple-darwin", "release");
        assert!(keys.contains(&"CC_aarch64_apple_darwin".to_string()));
        assert!(keys.contains(&"aarch64-apple-darwin_CFLAGS".to_string()));
        assert!(keys.contains(&"CARGO_PROFILE_RELEASE_LTO".to_string()));
    }
}
