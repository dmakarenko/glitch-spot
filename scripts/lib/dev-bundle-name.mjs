// ABOUTME: Builds the debug-bundle productName carrying the short commit SHA.
// ABOUTME: Pure transform + a small CLI used by scripts/bundle-dev.sh.
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export function devProductName(baseName, sha, dirty = false) {
  if (!baseName) throw new Error("baseName is required");
  if (!sha) throw new Error("sha is required");
  return `${baseName} (${dirty ? `${sha}-dirty` : sha})`;
}

// A distinct bundle id so debug builds get their own Privacy & Security entry
// instead of merging with the installed release app under the shared id.
export function devIdentifier(baseIdentifier) {
  if (!baseIdentifier) throw new Error("baseIdentifier is required");
  const lastDot = baseIdentifier.lastIndexOf(".");
  const prefix = lastDot === -1 ? baseIdentifier : baseIdentifier.slice(0, lastDot);
  return `${prefix}.dev`;
}

function readConf(root) {
  const conf = JSON.parse(
    readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
  );
  if (!conf.productName) throw new Error("tauri.conf.json has no productName");
  if (!conf.identifier) throw new Error("tauri.conf.json has no identifier");
  return conf;
}

function git(root, args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

// CLI: prints the inline `--config` JSON for a SHA-suffixed debug bundle.
// pathToFileURL handles paths with spaces/escapes (import.meta.url is encoded).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = process.cwd();
  try {
    const conf = readConf(root);
    const productName = devProductName(
      conf.productName,
      git(root, ["rev-parse", "--short", "HEAD"]),
      git(root, ["status", "--porcelain"]).length > 0,
    );
    process.stdout.write(
      JSON.stringify({
        productName,
        identifier: devIdentifier(conf.identifier),
      }),
    );
  } catch (err) {
    process.stderr.write(`${err.message}\n`);
    process.exit(1);
  }
}
