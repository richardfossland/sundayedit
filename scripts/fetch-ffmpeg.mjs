#!/usr/bin/env node
// Copy static ffmpeg + ffprobe into src-tauri/binaries/ with the Rust
// target-triple suffix Tauri's `externalBin` expects (e.g.
// ffmpeg-aarch64-apple-darwin). Run before `tauri build`, both locally and
// in CI (each platform's runner copies its own binaries).
//
// `--universal` (macOS only) additionally fetches the *other* architecture's
// binaries and lipo's them into fat `ffmpeg-universal-apple-darwin` /
// `ffprobe-universal-apple-darwin` sidecars for
// `tauri build --target universal-apple-darwin`. A universal Tauri build
// needs all three suffixes present: tauri-build validates the per-arch
// sidecar during each cargo slice (`TARGET`), and the bundler picks up the
// `-universal-apple-darwin` one.
//
// Binaries come from the `ffmpeg-static` + `@ffprobe-installer/ffprobe` npm
// packages — GPL/LGPL ffmpeg builds. See docs/DISTRIBUTION.md for the
// licensing note before any public release. The non-host arch is fetched
// from the same upstreams those packages install from (the ffmpeg-static
// GitHub release / the @ffprobe-installer per-arch npm tarball), pinned to
// the installed package versions.

import { execFileSync, execSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  copyFileSync,
  chmodSync,
  existsSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { gunzipSync } from "node:zlib";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const ffmpegSrc = require("ffmpeg-static");
const ffprobeSrc = require("@ffprobe-installer/ffprobe").path;

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "src-tauri", "binaries");

const universal = process.argv.includes("--universal");

// Rust host triple — what `externalBin` matches against.
const host = execSync("rustc -vV", { encoding: "utf8" })
  .split("\n")
  .find((l) => l.startsWith("host:"))
  .slice("host:".length)
  .trim();
const ext = host.includes("windows") ? ".exe" : "";

mkdirSync(outDir, { recursive: true });

for (const [name, src] of [
  ["ffmpeg", ffmpegSrc],
  ["ffprobe", ffprobeSrc],
]) {
  if (!src || !existsSync(src)) {
    console.error(
      `✗ ${name}: source binary missing (${src}). Run \`npm install\` first.`,
    );
    process.exit(1);
  }
  const dest = join(outDir, `${name}-${host}${ext}`);
  copyFileSync(src, dest);
  chmodSync(dest, 0o755);
  console.log(`✓ ${name} → src-tauri/binaries/${name}-${host}${ext}`);
}

// ── Universal macOS sidecars ─────────────────────────────────────────────────

if (universal) {
  if (!host.endsWith("-apple-darwin")) {
    console.error("✗ --universal is only supported on macOS hosts.");
    process.exit(1);
  }

  // node arch ↔ rust triple for the two macOS architectures.
  const archOf = (triple) => (triple.startsWith("aarch64") ? "arm64" : "x64");
  const otherTriple =
    host === "aarch64-apple-darwin"
      ? "x86_64-apple-darwin"
      : "aarch64-apple-darwin";
  const otherArch = archOf(otherTriple);

  const download = async (url) => {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`GET ${url} → ${res.status}`);
    return Buffer.from(await res.arrayBuffer());
  };

  const tmp = mkdtempSync(join(tmpdir(), "sundayedit-ffmpeg-"));
  try {
    // ffmpeg: ffmpeg-static's install script downloads
    // ffmpeg-<platform>-<arch>.gz from its GitHub release; fetch the other
    // arch from the same release tag the installed package pins.
    const ffmpegPkg = require("ffmpeg-static/package.json");
    const release = ffmpegPkg["ffmpeg-static"]["binary-release-tag"];
    const ffmpegUrl = `https://github.com/eugeneware/ffmpeg-static/releases/download/${release}/ffmpeg-darwin-${otherArch}.gz`;
    console.log(`… fetching ${ffmpegUrl}`);
    const ffmpegOther = join(outDir, `ffmpeg-${otherTriple}`);
    writeFileSync(ffmpegOther, gunzipSync(await download(ffmpegUrl)));
    chmodSync(ffmpegOther, 0o755);
    console.log(`✓ ffmpeg → src-tauri/binaries/ffmpeg-${otherTriple}`);

    // ffprobe: @ffprobe-installer ships per-arch npm packages; npm only
    // installs the host one, so pull the other arch's tarball from the
    // registry at the version the installed meta-package pins.
    const ffprobePkg = require("@ffprobe-installer/ffprobe/package.json");
    const pkgName = `darwin-${otherArch}`;
    const version =
      ffprobePkg.optionalDependencies[`@ffprobe-installer/${pkgName}`];
    const ffprobeUrl = `https://registry.npmjs.org/@ffprobe-installer/${pkgName}/-/${pkgName}-${version}.tgz`;
    console.log(`… fetching ${ffprobeUrl}`);
    const tarball = join(tmp, `${pkgName}.tgz`);
    writeFileSync(tarball, await download(ffprobeUrl));
    execFileSync("tar", ["-xzf", tarball, "-C", tmp]);
    const ffprobeOther = join(outDir, `ffprobe-${otherTriple}`);
    copyFileSync(join(tmp, "package", "ffprobe"), ffprobeOther);
    chmodSync(ffprobeOther, 0o755);
    console.log(`✓ ffprobe → src-tauri/binaries/ffprobe-${otherTriple}`);

    // lipo the two arches into the universal sidecars the bundler expects.
    for (const name of ["ffmpeg", "ffprobe"]) {
      const out = join(outDir, `${name}-universal-apple-darwin`);
      execFileSync("lipo", [
        "-create",
        "-output",
        out,
        join(outDir, `${name}-${host}`),
        join(outDir, `${name}-${otherTriple}`),
      ]);
      chmodSync(out, 0o755);
      const archs = execFileSync("lipo", ["-archs", out], {
        encoding: "utf8",
      }).trim();
      if (!archs.includes("x86_64") || !archs.includes("arm64")) {
        throw new Error(`${out} is not universal (archs: ${archs})`);
      }
      console.log(
        `✓ ${name} → src-tauri/binaries/${name}-universal-apple-darwin (${archs})`,
      );
    }
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}
