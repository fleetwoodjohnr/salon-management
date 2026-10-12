import { describe, expect, it } from "vitest";
import { fmtDistance, fromDisplay, KM_PER_MI, toDisplay } from "./distance";

describe("distance units", () => {
  it("converts exactly and round-trips", () => {
    expect(toDisplay(KM_PER_MI, "mi")).toBe(1);
    expect(fromDisplay(5, "mi")).toBeCloseTo(8.04672, 10);
    expect(toDisplay(8, "km")).toBe(8);
    for (const km of [0.5, 3.2, 8.04672, 25, 50]) expect(fromDisplay(toDisplay(km, "mi"), "mi")).toBeCloseTo(km, 12);
  });
  it("formats in the chosen unit", () => {
    expect(fmtDistance(8.04672, "mi")).toBe("5.0 mi");
    expect(fmtDistance(8.04672, "km")).toBe("8.0 km");
    expect(fmtDistance(null, "mi")).toBe("—");
  });
});
