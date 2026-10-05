import { describe, expect, it } from "vitest";
import { money, num, roundDec, unitMoney } from "./format";

describe("roundDec", () => {
  it("rounds half away from zero without float error", () => {
    expect(roundDec("2.675", 2)).toBe("2.68");
    expect(roundDec("0.125", 2)).toBe("0.13");
    expect(roundDec("-0.125", 2)).toBe("-0.13");
    expect(roundDec("9.995", 2)).toBe("10.00");
    expect(roundDec("1.5", 0)).toBe("2");
    expect(roundDec("-0.001", 2)).toBe("0.00");
    expect(roundDec("12.666666666666666666666666667", 2)).toBe("12.67");
  });
});

describe("display", () => {
  it("formats money", () => {
    expect(money("1234.5")).toBe("$1,234.50");
    expect(money("-3")).toBe("−$3.00");
    expect(money(null)).toBe("—");
  });
  it("formats unit costs with extra precision", () => {
    expect(unitMoney("0.025360")).toBe("$0.0254");
    expect(unitMoney("0.75")).toBe("$0.75");
  });
  it("trims numbers", () => {
    expect(num("120.000")).toBe("120");
    expect(num("1234.567", 2)).toBe("1,234.57");
  });
});
