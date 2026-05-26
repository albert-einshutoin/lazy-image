// src/engine/memory/cgroup.rs
//
// Container memory limit detection via cgroup v1/v2.
//
// All functions in this module are NAPI-only because the only consumer
// (`detect_available_memory` in the parent module) is itself NAPI-gated.

#![cfg(feature = "napi")]

use std::fs;

/// Parsed cgroup mount entry: the on-disk mount point and the relative root
/// that the kernel reports inside the namespace.
pub(super) struct CgroupMount {
    pub(super) mount_point: String,
    pub(super) root: String,
}

/// Detect memory limit from cgroup v2. Returns `None` when no limit is set
/// (e.g. the host) or when detection cannot be performed.
pub(super) fn detect_cgroup_v2_memory() -> Option<u64> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok();
    let mount = mountinfo
        .as_deref()
        .and_then(parse_cgroup2_mount_point)
        .unwrap_or_else(|| CgroupMount {
            mount_point: "/sys/fs/cgroup".to_string(),
            root: "/".to_string(),
        });

    let rel_path = fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|c| parse_cgroup2_relative_path(&c))
        .unwrap_or_default();

    let rel = strip_mount_root(&mount.root, &rel_path);
    let path = join_mount_rel_file(&mount.mount_point, &rel, "memory.max");
    if let Ok(content) = fs::read_to_string(&path) {
        let trimmed = content.trim();
        if trimmed == "max" {
            return None;
        }
        if let Ok(memory) = trimmed.parse::<u64>() {
            return Some(memory);
        }
    }
    None
}

/// Detect memory limit from cgroup v1.
pub(super) fn detect_cgroup_v1_memory() -> Option<u64> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok();
    let mount = mountinfo
        .as_deref()
        .and_then(|m| parse_cgroup1_mount_point(m, "memory"))
        .unwrap_or_else(|| CgroupMount {
            mount_point: "/sys/fs/cgroup/memory".to_string(),
            root: "/".to_string(),
        });

    let rel_path = fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|c| parse_cgroup1_memory_relative_path(&c))
        .unwrap_or_default();

    let rel = strip_mount_root(&mount.root, &rel_path);
    let path = join_mount_rel_file(&mount.mount_point, &rel, "memory.limit_in_bytes");

    if let Ok(content) = fs::read_to_string(&path) {
        let trimmed = content.trim();
        if let Ok(memory) = trimmed.parse::<u64>() {
            // Very large values (like 2^63-1) usually mean "no limit"
            if memory > 1_000_000_000_000_000 {
                return None; // No limit, fall back to system memory
            }
            return Some(memory);
        }
    }

    None
}

fn parse_cgroup2_mount_point(mountinfo: &str) -> Option<CgroupMount> {
    for line in mountinfo.lines() {
        // fields: id parent major:minor root mountpoint opts ... - fstype ...
        // Example: 36 27 0:31 / /sys/fs/cgroup rw,relatime - cgroup2 cgroup2 rw
        let mut parts = line.split(" - ");
        let pre = parts.next()?;
        let post = parts.next()?;
        if !post.contains("cgroup2") {
            continue;
        }
        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() >= 5 {
            return Some(CgroupMount {
                root: pre_fields[3].to_string(),
                mount_point: pre_fields[4].to_string(),
            });
        }
    }
    None
}

fn parse_cgroup1_mount_point(mountinfo: &str, controller: &str) -> Option<CgroupMount> {
    for line in mountinfo.lines() {
        let mut parts = line.split(" - ");
        let pre = parts.next()?;
        let post = parts.next()?;
        if !(post.contains("cgroup") && post.contains(controller)) {
            continue;
        }
        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() >= 5 {
            return Some(CgroupMount {
                root: pre_fields[3].to_string(),
                mount_point: pre_fields[4].to_string(),
            });
        }
    }
    None
}

fn parse_cgroup2_relative_path(content: &str) -> Option<String> {
    // Format: 0::/docker/abcd...
    // cgroup v2 uses hierarchy ID "0" for the unified hierarchy.
    // Skip non-v2 lines (e.g. v1 entries with hierarchy ID > 0).
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let hier = match parts.next() {
            Some(h) => h,
            None => continue,
        };
        let _controllers = match parts.next() {
            Some(c) => c,
            None => continue,
        };
        let path = match parts.next() {
            Some(p) => p,
            None => continue,
        };
        if hier == "0" {
            return Some(path.to_string());
        }
    }
    None
}

fn parse_cgroup1_memory_relative_path(content: &str) -> Option<String> {
    // Lines like: 5:memory:/kubepods.slice/... or 5:memory:/
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let _id = parts.next()?;
        let controllers = parts.next()?;
        if controllers.split(',').any(|c| c == "memory") {
            let path = parts.next().unwrap_or("");
            return Some(path.to_string());
        }
    }
    None
}

fn strip_mount_root(root: &str, rel: &str) -> String {
    if root == "/" {
        return rel.to_string();
    }
    let trimmed_root = root.trim_end_matches('/');
    let rel_no_leading = rel.trim_start_matches('/');
    let prefix = trimmed_root.trim_start_matches('/');
    if rel_no_leading.starts_with(prefix) {
        let stripped = rel_no_leading
            .trim_start_matches(prefix)
            .trim_start_matches('/');
        stripped.to_string()
    } else {
        rel.to_string()
    }
}

fn join_mount_rel_file(mount_point: &str, rel: &str, file: &str) -> String {
    let base = mount_point.trim_end_matches('/');
    if rel.is_empty() {
        return format!("{}/{}", base, file);
    }
    format!("{}/{}/{}", base, rel.trim_start_matches('/'), file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cgroup2_mount_point() {
        let sample = "36 27 0:31 / /sys/fs/cgroup rw,relatime - cgroup2 cgroup2 rw\n";
        let mount = parse_cgroup2_mount_point(sample).unwrap();
        assert_eq!(mount.mount_point, "/sys/fs/cgroup");
        assert_eq!(mount.root, "/");
    }

    #[test]
    fn test_parse_cgroup1_memory_relative_path() {
        let sample =
            "5:memory:/kubepods.slice/kubepods-besteffort.slice\n9:cpu,cpuacct:/kubepods.slice\n";
        assert_eq!(
            parse_cgroup1_memory_relative_path(sample),
            Some("/kubepods.slice/kubepods-besteffort.slice".to_string())
        );
    }

    #[test]
    fn test_strip_mount_root_removes_duplicate() {
        let root = "/docker/12345";
        let rel = "/docker/12345/foo/bar";
        assert_eq!(strip_mount_root(root, rel), "foo/bar");
    }

    #[test]
    fn test_join_mount_rel_file_handles_empty_rel() {
        let path = join_mount_rel_file("/sys/fs/cgroup", "", "memory.max");
        assert_eq!(path, "/sys/fs/cgroup/memory.max");
    }

    #[test]
    fn test_parse_cgroup2_relative_path() {
        let sample = "0::/docker/abcd\n";
        assert_eq!(
            parse_cgroup2_relative_path(sample),
            Some("/docker/abcd".to_string())
        );
    }

    #[test]
    fn test_parse_cgroup2_relative_path_mixed_v1_v2() {
        // cgroup v1 lines appear before the v2 unified line
        let sample = "12:memory:/docker/abc123\n11:cpuset:/docker/abc123\n0::/kubepods/pod-xyz\n";
        assert_eq!(
            parse_cgroup2_relative_path(sample),
            Some("/kubepods/pod-xyz".to_string())
        );
    }

    #[test]
    fn test_parse_cgroup2_relative_path_empty() {
        assert_eq!(parse_cgroup2_relative_path(""), None);
    }

    #[test]
    fn test_parse_cgroup2_relative_path_no_v2_entry() {
        // Only cgroup v1 lines, no hierarchy 0
        let sample = "5:memory:/docker/abc\n3:cpuset:/docker/abc\n";
        assert_eq!(parse_cgroup2_relative_path(sample), None);
    }

    #[test]
    fn test_parse_cgroup2_relative_path_root() {
        let sample = "0::/\n";
        assert_eq!(parse_cgroup2_relative_path(sample), Some("/".to_string()));
    }
}
