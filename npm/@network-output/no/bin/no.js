#!/usr/bin/env node

const { execFileSync } = require("child_process");
const { existsSync, readFileSync } = require("fs");
const path = require("path");

const PLATFORM_PACKAGES = {
  "darwin arm64": "@network-output/no-darwin-arm64",
  "darwin x64": "@network-output/no-darwin-x64",
  "linux arm64": "@network-output/no-linux-arm64",
  "linux x64": "@network-output/no-linux-x64",
  "win32 arm64": "@network-output/no-win32-arm64",
  "win32 x64": "@network-output/no-win32-x64",
};

function isMusl() {
  if (process.platform !== "linux") {
    return false;
  }

  try {
    const osRelease = readFileSync("/etc/os-release", "utf8");
    if (/alpine/i.test(osRelease)) {
      return true;
    }
  } catch {}

  try {
    const lddOutput = execFileSync("ldd", ["--version"], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    if (/musl/i.test(lddOutput)) {
      return true;
    }
  } catch (e) {
    const stderr = e.stderr ? e.stderr.toString() : "";
    if (/musl/i.test(stderr)) {
      return true;
    }
  }

  return false;
}

function resolvePackage() {
  const platform = process.platform;
  const arch = process.arch;

  let key = `${platform} ${arch}`;
  let packageName = PLATFORM_PACKAGES[key];

  if (!packageName) {
    console.error(
      `Error: Unsupported platform "${platform}" with architecture "${arch}".`
    );
    console.error(
      "Supported platforms: darwin (arm64, x64), linux (arm64, x64), win32 (arm64, x64)."
    );
    process.exit(1);
  }

  if (platform === "linux" && isMusl()) {
    packageName = packageName.replace(
      `no-linux-${arch}`,
      `no-linux-${arch}-musl`
    );
  }

  return packageName;
}

function findBinary(packageName) {
  const binaryName = process.platform === "win32" ? "no.exe" : "no";

  try {
    const packageDir = path.dirname(require.resolve(`${packageName}/package.json`));
    const binaryPath = path.join(packageDir, binaryName);
    if (existsSync(binaryPath)) {
      return binaryPath;
    }
  } catch {}

  console.error(`Error: Could not find the binary for package "${packageName}".`);
  console.error("");
  console.error("This usually means the platform-specific package was not installed.");
  console.error("Try reinstalling with:");
  console.error("");
  console.error("  npm install network-output");
  console.error("");
  process.exit(1);
}

const packageName = resolvePackage();
const binaryPath = findBinary(packageName);

try {
  execFileSync(binaryPath, process.argv.slice(2), {
    stdio: "inherit",
  });
} catch (e) {
  if (e.status !== null) {
    process.exit(e.status);
  }
  throw e;
}
