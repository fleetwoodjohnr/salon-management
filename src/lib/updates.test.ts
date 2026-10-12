import { describe, expect, it } from "vitest";
import { explainUpdateError, fmtBytes } from "./updates";

describe("update errors", () => {
  it("says nothing changed when a download fails or can't be verified", () => {
    expect(explainUpdateError("error sending request for url", "download")).toMatch(/interrupted. Nothing was changed/);
    expect(explainUpdateError("Invalid signature", "install")).toMatch(/couldn't be verified/);
    expect(explainUpdateError("Authentication failed or was cancelled", "install")).toMatch(/cancelled/);
    expect(explainUpdateError("Could not fetch a valid release JSON from the remote", "check")).toMatch(/No update information/);
    expect(explainUpdateError("temp directory is not on the same mount point as the AppImage", "install")).toMatch(/couldn't replace/);
  });
  it("formats sizes", () => {
    expect(fmtBytes(512)).toBe("512 B");
    expect(fmtBytes(15 * 1024 * 1024)).toBe("15.0 MB");
  });
});
