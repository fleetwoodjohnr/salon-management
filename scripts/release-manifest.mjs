#!/usr/bin/env node
// Builds the updater manifest (latest.json) and release notes for a GitHub Release.
//   node scripts/release-manifest.mjs <installers-dir> <tag, e.g. v0.2.0> <owner/repo>
// Reads each installer's .sig (written by `tauri build` with createUpdaterArtifacts), renames every
// file to a space-free name (GitHub turns spaces in asset names into dots, which would break the
// URLs), gives the macOS updater archive a versioned name, and writes latest.json and
// release-notes.md in the current folder. Fails if any platform is missing, so a
// release can't go out that some installed copies couldn't update from.
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function listFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? listFiles(path.join(dir, e.name)) : [path.join(dir, e.name)]));
}

/** The "## [0.2.0] …" section of CHANGELOG.md, without its heading. */
export function changelogSection(text, version) {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => new RegExp(`^##\\s+\\[?v?${version.replace(/\./g, "\\.")}\\]?(\\s|$)`).test(l));
  if (start < 0) return "";
  const end = lines.findIndex((l, i) => i > start && /^##\s/.test(l));
  return lines.slice(start + 1, end < 0 ? undefined : end).join("\n").trim();
}

export function buildManifest({ files, version, repo, tag, notes, pubDate }) {
  const pick = (label, re) => {
    const f = files.find((x) => re.test(path.basename(x)) && path.basename(x).includes(version));
    if (!f) throw new Error(`No ${label} installer found for the release.`);
    if (!files.includes(`${f}.sig`)) throw new Error(`${path.basename(f)} has no .sig signature. Set the TAURI_SIGNING_PRIVATE_KEY secret so CI signs updater artifacts.`);
    return { signature: fs.readFileSync(`${f}.sig`, "utf8").trim(), url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(path.basename(f))}` };
  };
  const appimage = pick("AppImage", /\.AppImage$/);
  const deb = pick(".deb", /\.deb$/);
  const rpm = pick(".rpm", /\.rpm$/);
  const nsis = pick("Windows setup", /-setup\.exe$/);
  const msi = pick("MSI", /\.msi$/);
  const mac = pick("macOS updater archive", /\.app\.tar\.gz$/);
  return {
    version,
    notes,
    pub_date: pubDate,
    // The updater looks up "<os>-<arch>-<how it was installed>" first, then "<os>-<arch>".
    platforms: {
      "linux-x86_64": appimage,
      "linux-x86_64-appimage": appimage,
      "linux-x86_64-deb": deb,
      "linux-x86_64-rpm": rpm,
      "windows-x86_64": nsis,
      "windows-x86_64-nsis": nsis,
      "windows-x86_64-msi": msi,
      "darwin-aarch64": mac,
      "darwin-aarch64-app": mac,
      "darwin-x86_64": mac,
      "darwin-x86_64-app": mac,
    },
  };
}

const INSTALL_NOTES = (repo) => `## Install

Already using Salon Resource Manager 0.2.0 or later? Open **Settings → Application updates** to install this version from inside the app. Your data is kept.

Otherwise, pick the file for your system:

| System | File |
|---|---|
| Windows 10/11 | \`…_x64-setup.exe\` (recommended) or \`.msi\` |
| macOS 11+ (Apple Silicon and Intel) | \`…_universal.dmg\` |
| Fedora | \`.rpm\` |
| Ubuntu 22.04+ / Debian | \`.deb\` |
| Other Linux | \`.AppImage\` |

The installers aren't code-signed by a certificate authority. Windows may say "Windows protected your PC": choose **More info → Run anyway**. On a Mac, the first open is blocked: go to **System Settings → Privacy & Security** and choose **Open Anyway**. In-app updates are verified with the project's own signing key. Full install notes: [README](https://github.com/${repo}#install).
`;

/** Space-free asset names ("Salon Resource Manager_0.2.0_amd64.deb" → "Salon-Resource-Manager_0.2.0_amd64.deb"). */
export function assetNames(dir, version) {
  for (const f of listFiles(dir)) {
    let name = path.basename(f).replace(/ /g, "-");
    if (/\.app\.tar\.gz(\.sig)?$/.test(name)) name = name.replace(/^.*?\.app\.tar\.gz/, `Salon-Resource-Manager_${version}_universal.app.tar.gz`);
    const to = path.join(path.dirname(f), name);
    if (to !== f) fs.renameSync(f, to);
  }
  return listFiles(dir);
}

function main([dir, tag, repo]) {
  if (!dir || !tag || !repo) throw new Error("usage: release-manifest.mjs <installers-dir> <tag> <owner/repo>");
  const version = tag.replace(/^v/, "");
  // Only this version's files are released (a build cache could hold older ones).
  const files = assetNames(dir, version).filter((f) => {
    if (path.basename(f).includes(version)) return true;
    console.log(`not releasing ${path.basename(f)} (another version)`);
    fs.rmSync(f);
    return false;
  });
  const notes = changelogSection(fs.existsSync("CHANGELOG.md") ? fs.readFileSync("CHANGELOG.md", "utf8") : "", version) || `Salon Resource Manager ${version}`;
  const manifest = buildManifest({ files, version, repo, tag, notes, pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z") });
  fs.writeFileSync("latest.json", JSON.stringify(manifest, null, 2) + "\n");
  fs.writeFileSync("release-notes.md", `## What's new\n\n${notes}\n\n${INSTALL_NOTES(repo)}`);
  console.log(`latest.json for ${version}: ${Object.keys(manifest.platforms).length} platform entries`);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`::error::${e.message}`);
    process.exit(1);
  }
}
