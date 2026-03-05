use crate::cli::SkillsAction;
use crate::error::{ErrorCode, NetError};
use crate::output::Protocol;
use std::fs;
use std::path::{Path, PathBuf};

const PLUGIN_FILES: &[(&str, &str)] = &[
  (
    ".claude-plugin/plugin.json",
    include_str!("../../claude-plugin/.claude-plugin/plugin.json"),
  ),
  ("README.md", include_str!("../../claude-plugin/README.md")),
  ("LICENSE", include_str!("../../claude-plugin/LICENSE")),
  (
    "skills/http-requests/SKILL.md",
    include_str!("../../claude-plugin/skills/http-requests/SKILL.md"),
  ),
  (
    "skills/websocket-debugging/SKILL.md",
    include_str!("../../claude-plugin/skills/websocket-debugging/SKILL.md"),
  ),
  (
    "skills/network-diagnostics/SKILL.md",
    include_str!("../../claude-plugin/skills/network-diagnostics/SKILL.md"),
  ),
  (
    "skills/mqtt-messaging/SKILL.md",
    include_str!("../../claude-plugin/skills/mqtt-messaging/SKILL.md"),
  ),
  (
    "skills/tcp-udp-testing/SKILL.md",
    include_str!("../../claude-plugin/skills/tcp-udp-testing/SKILL.md"),
  ),
  (
    "skills/sse-monitoring/SKILL.md",
    include_str!("../../claude-plugin/skills/sse-monitoring/SKILL.md"),
  ),
  (
    "skills/output-filtering/SKILL.md",
    include_str!("../../claude-plugin/skills/output-filtering/SKILL.md"),
  ),
  (
    "references/cli-reference.md",
    include_str!("../../claude-plugin/references/cli-reference.md"),
  ),
  (
    "references/output-schema.md",
    include_str!("../../claude-plugin/references/output-schema.md"),
  ),
  (
    "references/error-codes.md",
    include_str!("../../claude-plugin/references/error-codes.md"),
  ),
];

fn home_dir() -> Result<PathBuf, NetError> {
  std::env::var("HOME")
    .or_else(|_| std::env::var("USERPROFILE"))
    .map(PathBuf::from)
    .map_err(|_| NetError::new(ErrorCode::IoError, "could not determine home directory", Protocol::Http))
}

fn write_plugin(target: &Path) -> Result<(), NetError> {
  for (rel_path, content) in PLUGIN_FILES {
    let dest = target.join(rel_path);
    if let Some(parent) = dest.parent() {
      fs::create_dir_all(parent).map_err(|e| {
        NetError::new(
          ErrorCode::IoError,
          format!("failed to create directory {}: {e}", parent.display()),
          Protocol::Http,
        )
      })?;
    }
    fs::write(&dest, content).map_err(|e| {
      NetError::new(
        ErrorCode::IoError,
        format!("failed to write {}: {e}", dest.display()),
        Protocol::Http,
      )
    })?;
  }
  Ok(())
}

pub async fn run(action: SkillsAction) -> Result<(), NetError> {
  match action {
    SkillsAction::Install(args) => {
      let target = match args.path {
        Some(p) => PathBuf::from(p),
        None => home_dir()?.join(".claude/plugins/no"),
      };
      write_plugin(&target)?;
      println!("Skills installed to {}", target.display());
    }
    SkillsAction::Export(args) => {
      let target = PathBuf::from(&args.path);
      write_plugin(&target)?;
      println!("Skills exported to {}", target.display());
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn plugin_files_not_empty() {
    for (path, content) in PLUGIN_FILES {
      assert!(!content.is_empty(), "embedded file {path} should not be empty");
    }
  }

  #[test]
  fn plugin_files_count() {
    assert_eq!(PLUGIN_FILES.len(), 13);
  }

  #[tokio::test]
  async fn install_to_temp_dir() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().to_path_buf();
    let action = SkillsAction::Install(crate::cli::SkillsInstallArgs {
      path: Some(target.to_string_lossy().into_owned()),
    });
    run(action).await.unwrap();

    assert!(target.join(".claude-plugin/plugin.json").exists());
    assert!(target.join("skills/http-requests/SKILL.md").exists());
    assert!(target.join("references/cli-reference.md").exists());
    assert!(target.join("README.md").exists());
    assert!(target.join("LICENSE").exists());
  }

  #[tokio::test]
  async fn export_to_temp_dir() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("exported");
    let action = SkillsAction::Export(crate::cli::SkillsExportArgs {
      path: target.to_string_lossy().into_owned(),
    });
    run(action).await.unwrap();

    assert!(target.join(".claude-plugin/plugin.json").exists());
    assert!(target.join("skills/mqtt-messaging/SKILL.md").exists());
    assert!(target.join("references/error-codes.md").exists());
  }
}
