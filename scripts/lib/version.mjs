// ABOUTME: Computes the next semver and writes it across all version-bearing files.
// ABOUTME: Pure transforms + a small CLI used by scripts/release.sh.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const FILES = {
  packageJson: "package.json",
  packageLock: "package-lock.json",
  tauriConf: "src-tauri/tauri.conf.json",
  cargoToml: "src-tauri/Cargo.toml",
  cargoLock: "src-tauri/Cargo.lock",
};

export function nextVersion(current, bump) {
  if (/^\d+\.\d+\.\d+$/.test(bump)) return bump;
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
  if (!m) throw new Error(`Invalid current version: ${current}`);
  const [major, minor, patch] = m.slice(1).map(Number);
  if (bump === "major") return `${major + 1}.0.0`;
  if (bump === "minor") return `${major}.${minor + 1}.0`;
  if (bump === "patch") return `${major}.${minor}.${patch + 1}`;
  throw new Error(`Invalid bump: ${bump} (expected patch|minor|major|X.Y.Z)`);
}

export function setJsonVersion(text, version) {
  const out = text.replace(/("version"\s*:\s*)"[^"]*"/, `$1"${version}"`);
  if (out === text) throw new Error('No "version" field found in JSON');
  return out;
}

export function setCargoTomlVersion(text, version) {
  // The first line-anchored `version = "..."` is the [package] version;
  // dependency versions are inline (`foo = { version = ... }`) or in later tables.
  const out = text.replace(/^version = "[^"]*"$/m, `version = "${version}"`);
  if (out === text) throw new Error("No package version found in Cargo.toml");
  return out;
}

export function setCargoLockVersion(text, version, pkg = "glitch_spot") {
  const re = new RegExp(`(name = "${pkg}"\\nversion = )"[^"]*"`);
  const out = text.replace(re, `$1"${version}"`);
  if (out === text) throw new Error(`No ${pkg} entry found in Cargo.lock`);
  return out;
}

export function setPackageLockVersion(text, version) {
  // npm lockfiles carry the root version twice: top-level and under packages[""].
  // A round-trip reproduces npm's exact formatting, so only the version lines change.
  const lock = JSON.parse(text);
  let changed = false;
  if (typeof lock.version === "string") {
    lock.version = version;
    changed = true;
  }
  if (lock.packages?.[""] && typeof lock.packages[""].version === "string") {
    lock.packages[""].version = version;
    changed = true;
  }
  if (!changed) throw new Error("No root version field found in package-lock.json");
  return JSON.stringify(lock, null, 2) + "\n";
}

function readCurrent(root) {
  const conf = JSON.parse(readFileSync(join(root, FILES.tauriConf), "utf8"));
  if (!conf.version) throw new Error("tauri.conf.json has no version");
  return conf.version;
}

export function applyVersion(version, root) {
  const edits = [
    [FILES.packageJson, setJsonVersion],
    [FILES.packageLock, setPackageLockVersion],
    [FILES.tauriConf, setJsonVersion],
    [FILES.cargoToml, setCargoTomlVersion],
    [FILES.cargoLock, setCargoLockVersion],
  ];
  for (const [rel, fn] of edits) {
    const p = join(root, rel);
    writeFileSync(p, fn(readFileSync(p, "utf8"), version));
  }
}

// CLI: `node scripts/lib/version.mjs current | next <bump> | apply <bump>`
if (import.meta.url === `file://${process.argv[1]}`) {
  const root = process.cwd();
  const [cmd, arg] = process.argv.slice(2);
  try {
    if (cmd === "current") {
      process.stdout.write(readCurrent(root));
    } else if (cmd === "next") {
      process.stdout.write(nextVersion(readCurrent(root), arg));
    } else if (cmd === "apply") {
      const v = nextVersion(readCurrent(root), arg);
      applyVersion(v, root);
      process.stdout.write(v);
    } else {
      process.stderr.write(
        "usage: version.mjs current | next <bump> | apply <bump>\n",
      );
      process.exit(2);
    }
  } catch (err) {
    process.stderr.write(`${err.message}\n`);
    process.exit(1);
  }
}
