//! Build script: generates the replay protobuf types from `proto/replay.proto` and records the
//! git commit the engine is built from (`replay::engine_version`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    generate_replay_proto();
    stamp_git_commit();
}

/// Compile `proto/replay.proto` with `protox` (pure Rust, so no system `protoc` is needed) and
/// generate its Rust types with `prost-build`.
fn generate_replay_proto() {
    println!("cargo:rerun-if-changed=proto/replay.proto");
    let descriptors =
        protox::compile(["replay.proto"], ["proto"]).expect("proto/replay.proto should compile");
    prost_build::Config::new()
        // Ordered maps, so encoding a replay is deterministic.
        .btree_map(["."])
        .compile_fds(descriptors)
        .expect("generating the replay protobuf types should succeed");
}

/// Set `SEAGLASS_GIT_COMMIT` to the checked-out commit (`unknown` outside a git checkout).
fn stamp_git_commit() {
    let commit = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=SEAGLASS_GIT_COMMIT={commit}");

    // Re-run when HEAD moves: a checkout rewrites HEAD, a commit rewrites the branch ref.
    let dirs: Vec<PathBuf> = [
        git(&["rev-parse", "--absolute-git-dir"]),
        git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();
    let head_ref = git(&["symbolic-ref", "-q", "HEAD"]);
    for dir in &dirs {
        rerun_if_exists(&dir.join("HEAD"));
        rerun_if_exists(&dir.join("packed-refs"));
        if let Some(head_ref) = &head_ref {
            rerun_if_exists(&dir.join(head_ref));
        }
    }
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn rerun_if_exists(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
