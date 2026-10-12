import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { assetNames, buildManifest, changelogSection, listFiles } from "./release-manifest.mjs";

function fakeRelease(withSigs = true) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "srm-release-"));
  for (const f of ["linux/x_0.2.0_amd64.AppImage", "linux/x_0.2.0_amd64.deb", "linux/x-0.2.0-1.x86_64.rpm", "linux/x-0.1.0-1.x86_64.rpm", "win/x_0.2.0_x64-setup.exe", "win/x_0.2.0_x64_en-US.msi", "mac/Salon-Resource-Manager_0.2.0_universal.app.tar.gz", "mac/x_0.2.0.dmg"]) {
    fs.mkdirSync(path.join(dir, path.dirname(f)), { recursive: true });
    fs.writeFileSync(path.join(dir, f), "bin");
    if (withSigs && !f.endsWith(".dmg")) fs.writeFileSync(path.join(dir, `${f}.sig`), `sig-of-${path.basename(f)}\n`);
  }
  return dir;
}

describe("release manifest", () => {
  it("maps every platform and install type to its signed file", () => {
    const dir = fakeRelease();
    const m = buildManifest({ files: listFiles(dir), version: "0.2.0", repo: "o/r", tag: "v0.2.0", notes: "n", pubDate: "2026-10-11T00:00:00Z" });
    expect(m.platforms["linux-x86_64-deb"].url).toBe("https://github.com/o/r/releases/download/v0.2.0/x_0.2.0_amd64.deb");
    expect(m.platforms["linux-x86_64-deb"].signature).toBe("sig-of-x_0.2.0_amd64.deb");
    expect(m.platforms["linux-x86_64-rpm"].url).toMatch(/x-0\.2\.0-1\.x86_64\.rpm$/); // never the older 0.1.0 file
    expect(m.platforms["linux-x86_64"].url).toMatch(/\.AppImage$/);
    expect(m.platforms["windows-x86_64-msi"].url).toMatch(/\.msi$/);
    expect(m.platforms["darwin-aarch64-app"]).toEqual(m.platforms["darwin-x86_64"]);
    expect(Object.keys(m.platforms)).toHaveLength(11);
  });
  it("refuses a release without signatures", () => {
    expect(() => buildManifest({ files: listFiles(fakeRelease(false)), version: "0.2.0", repo: "o/r", tag: "v0.2.0", notes: "", pubDate: "" })).toThrow(/TAURI_SIGNING_PRIVATE_KEY/);
  });
  it("gives real artifact names space-free asset names", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "srm-names-"));
    for (const f of ["Salon Resource Manager_0.2.0_amd64.deb", "Salon Resource Manager_0.2.0_amd64.deb.sig", "Salon Resource Manager.app.tar.gz", "Salon Resource Manager.app.tar.gz.sig"]) fs.writeFileSync(path.join(dir, f), "x");
    const names = assetNames(dir, "0.2.0").map((f) => path.basename(f)).sort();
    expect(names).toEqual(["Salon-Resource-Manager_0.2.0_amd64.deb", "Salon-Resource-Manager_0.2.0_amd64.deb.sig", "Salon-Resource-Manager_0.2.0_universal.app.tar.gz", "Salon-Resource-Manager_0.2.0_universal.app.tar.gz.sig"]);
  });
  it("takes the version's changelog section", () => {
    const text = "# Changelog\n\n## [0.2.0] - 2026-10-11\n\n- New thing\n\n## [0.1.0]\n\n- Old thing\n";
    expect(changelogSection(text, "0.2.0")).toBe("- New thing");
    expect(changelogSection(text, "0.3.0")).toBe("");
  });
});
