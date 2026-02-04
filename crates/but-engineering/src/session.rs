//! Session-based self-identification via ancestor PID.
//!
//! Hooks don't know which agent they belong to. This module provides a
//! reliable mechanism: walk the process tree from the current PID upward
//! to find the Claude Code process, then look up the associated agent-id
//! in the sessions table.

use std::collections::HashMap;

/// Find the PID of the nearest "claude" ancestor process.
///
/// Runs a single `ps -ax -o pid= -o ppid= -o comm=` call, builds an
/// in-memory process tree, and walks upward from the current PID looking
/// for a process whose command name is "claude".
///
/// Returns `None` if no claude ancestor is found or if `ps` fails.
pub fn find_claude_ancestor() -> Option<u32> {
    let output = std::process::Command::new("ps")
        .args(["-ax", "-o", "pid=", "-o", "ppid=", "-o", "comm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Build pid → (ppid, comm) map.
    let mut tree: HashMap<u32, (u32, String)> = HashMap::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let pid: u32 = match parts[0].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let ppid: u32 = match parts[1].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        // comm may contain spaces; rejoin everything after pid and ppid.
        let comm = parts[2..].join(" ");
        tree.insert(pid, (ppid, comm));
    }

    // Walk upward from our parent PID (skip self — we're but-engineering, not claude).
    let self_pid = std::process::id();
    let mut current = match tree.get(&self_pid) {
        Some((ppid, _)) => *ppid,
        None => return None,
    };

    // Guard against cycles: limit iterations to tree size.
    let max_depth = tree.len();
    for _ in 0..max_depth {
        let (ppid, comm) = tree.get(&current)?;

        // Check if this process is "claude".
        // The comm field is the executable basename (or full path on some systems).
        let basename = comm.rsplit('/').next().unwrap_or(comm);
        if basename == "claude" {
            return Some(current);
        }

        // Move to parent. Stop if we hit PID 0/1 (init/launchd).
        if *ppid == 0 || *ppid == current {
            return None;
        }
        current = *ppid;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_claude_ancestor_does_not_panic() {
        // In test context, we probably don't run under Claude Code, so this
        // should return None. We can't assert that on all systems though
        // (a developer might be running under claude).
        let _ = find_claude_ancestor();
    }
}
