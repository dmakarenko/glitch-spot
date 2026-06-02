// ABOUTME: Unit tests for the release version helper (compute + multi-file sync).
// ABOUTME: Runs under the project's Vitest config (scripts/**/*.test.ts).
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  nextVersion,
  setJsonVersion,
  setCargoTomlVersion,
  setCargoLockVersion,
  setPackageLockVersion,
  applyVersion,
} from "./version.mjs";

describe("nextVersion", () => {
  it("bumps patch, minor, major", () => {
    expect(nextVersion("0.1.0", "patch")).toBe("0.1.1");
    expect(nextVersion("0.1.0", "minor")).toBe("0.2.0");
    expect(nextVersion("0.1.9", "major")).toBe("1.0.0");
  });
  it("accepts an explicit X.Y.Z", () => {
    expect(nextVersion("0.1.0", "2.3.4")).toBe("2.3.4");
  });
  it("rejects an invalid bump or current version", () => {
    expect(() => nextVersion("0.1.0", "huge")).toThrow();
    expect(() => nextVersion("not-a-version", "patch")).toThrow();
  });
});

describe("setJsonVersion", () => {
  it("changes only the version field", () => {
    const src = '{\n  "name": "x",\n  "version": "0.0.0",\n  "scripts": {}\n}\n';
    const out = setJsonVersion(src, "1.2.3");
    expect(JSON.parse(out).version).toBe("1.2.3");
    expect(out).toContain('"name": "x"');
  });
  it("throws when no version field exists", () => {
    expect(() => setJsonVersion('{\n  "name": "x"\n}\n', "1.2.3")).toThrow();
  });
});

describe("setCargoTomlVersion", () => {
  it("changes the package version, not dependency versions", () => {
    const src =
      '[package]\nname = "glitch_spot"\nversion = "0.1.0"\n\n' +
      '[dependencies]\nfoo = { version = "9.9.9" }\n';
    const out = setCargoTomlVersion(src, "1.2.3");
    expect(out).toContain('version = "1.2.3"');
    expect(out).toContain('foo = { version = "9.9.9" }');
  });
  it("throws when no package version exists", () => {
    expect(() => setCargoTomlVersion('[package]\nname = "x"\n', "1.2.3")).toThrow();
  });
});

describe("setCargoLockVersion", () => {
  it("targets the named package entry only", () => {
    const src =
      'name = "other"\nversion = "5.0.0"\n\n' +
      'name = "glitch_spot"\nversion = "0.1.0"\ndependencies = [\n';
    const out = setCargoLockVersion(src, "1.2.3");
    expect(out).toContain('name = "glitch_spot"\nversion = "1.2.3"');
    expect(out).toContain('name = "other"\nversion = "5.0.0"');
  });
  it("throws when the package entry is absent", () => {
    expect(() =>
      setCargoLockVersion('name = "other"\nversion = "5.0.0"\n', "1.2.3"),
    ).toThrow();
  });
});

describe("setPackageLockVersion", () => {
  it("updates both root version fields, not dependency versions", () => {
    const src =
      '{\n  "name": "glitch-spot",\n  "version": "0.0.0",\n  "lockfileVersion": 3,\n' +
      '  "packages": {\n    "": {\n      "name": "glitch-spot",\n      "version": "0.0.0"\n    },\n' +
      '    "node_modules/foo": {\n      "version": "9.9.9"\n    }\n  }\n}\n';
    const out = setPackageLockVersion(src, "1.2.3");
    const lock = JSON.parse(out);
    expect(lock.version).toBe("1.2.3");
    expect(lock.packages[""].version).toBe("1.2.3");
    expect(lock.packages["node_modules/foo"].version).toBe("9.9.9");
  });
  it("throws when no root version field exists", () => {
    expect(() =>
      setPackageLockVersion('{\n  "name": "x",\n  "packages": {}\n}\n', "1.2.3"),
    ).toThrow();
  });
});

describe("applyVersion", () => {
  let root: string;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "gs-ver-"));
    mkdirSync(join(root, "src-tauri"), { recursive: true });
    writeFileSync(join(root, "package.json"), '{\n  "version": "0.0.0"\n}\n');
    writeFileSync(
      join(root, "package-lock.json"),
      '{\n  "name": "glitch-spot",\n  "version": "0.0.0",\n  "lockfileVersion": 3,\n  "packages": {\n    "": {\n      "version": "0.0.0"\n    }\n  }\n}\n',
    );
    writeFileSync(
      join(root, "src-tauri/tauri.conf.json"),
      '{\n  "version": "0.1.0"\n}\n',
    );
    writeFileSync(
      join(root, "src-tauri/Cargo.toml"),
      '[package]\nname = "glitch_spot"\nversion = "0.1.0"\n',
    );
    writeFileSync(
      join(root, "src-tauri/Cargo.lock"),
      'name = "glitch_spot"\nversion = "0.1.0"\ndependencies = []\n',
    );
  });
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it("writes the same version into all version files", () => {
    applyVersion("1.2.3", root);
    const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
    const conf = JSON.parse(
      readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
    );
    const lock = JSON.parse(
      readFileSync(join(root, "package-lock.json"), "utf8"),
    );
    expect(pkg.version).toBe("1.2.3");
    expect(conf.version).toBe("1.2.3");
    expect(lock.version).toBe("1.2.3");
    expect(lock.packages[""].version).toBe("1.2.3");
    expect(readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8")).toContain(
      'version = "1.2.3"',
    );
    expect(readFileSync(join(root, "src-tauri/Cargo.lock"), "utf8")).toContain(
      'name = "glitch_spot"\nversion = "1.2.3"',
    );
  });
});
