default:
  just --list

build:
  cargo build

release:
  cargo build --release

test:
  cargo test

lint:
  cargo clippy -- -D warnings

fmt:
  cargo fmt

fmt-check:
  cargo fmt --check

check: fmt-check lint test

test-unit:
  cargo test --lib

test-integration:
  cargo test --test '*'

run *args:
  cargo run -- {{args}}

validate-plugin:
  #!/usr/bin/env bash
  set -euo pipefail
  echo "Validating plugin structure..."
  # Check plugin.json exists and is valid JSON
  if ! python3 -m json.tool claude-plugin/.claude-plugin/plugin.json > /dev/null 2>&1; then
    echo "FAIL: claude-plugin/.claude-plugin/plugin.json is missing or invalid JSON"
    exit 1
  fi
  echo "  plugin.json: OK"
  # Check all SKILL.md files exist and have YAML frontmatter
  skills=(
    "http-requests"
    "websocket-debugging"
    "network-diagnostics"
    "mqtt-messaging"
    "tcp-udp-testing"
    "sse-monitoring"
    "output-filtering"
  )
  for skill in "${skills[@]}"; do
    skill_file="claude-plugin/skills/${skill}/SKILL.md"
    if [ ! -f "${skill_file}" ]; then
      echo "FAIL: ${skill_file} not found"
      exit 1
    fi
    # Check YAML frontmatter (starts with ---)
    if ! head -1 "${skill_file}" | grep -q "^---$"; then
      echo "FAIL: ${skill_file} missing YAML frontmatter"
      exit 1
    fi
    # Check name in frontmatter matches directory name
    frontmatter_name=$(grep "^name:" "${skill_file}" | head -1 | sed 's/^name: *//')
    if [ "${frontmatter_name}" != "${skill}" ]; then
      echo "FAIL: ${skill_file} frontmatter name '${frontmatter_name}' does not match directory '${skill}'"
      exit 1
    fi
    echo "  ${skill}/SKILL.md: OK"
  done
  # Check reference files exist
  for ref in cli-reference.md output-schema.md error-codes.md; do
    if [ ! -f "claude-plugin/references/${ref}" ]; then
      echo "FAIL: claude-plugin/references/${ref} not found"
      exit 1
    fi
    echo "  references/${ref}: OK"
  done
  echo "Plugin validation passed."
