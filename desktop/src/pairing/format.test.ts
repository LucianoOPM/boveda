import { describe, expect, test } from "bun:test";

import { formatRemaining, isWarning, reasonText } from "./format";
import { END_REASONS } from "./types";

describe("formatRemaining", () => {
  test("muestra minutos y segundos", () => {
    expect(formatRemaining(105_000)).toBe("1:45");
    expect(formatRemaining(120_000)).toBe("2:00");
    expect(formatRemaining(9_000)).toBe("0:09");
  });

  test("redondea hacia arriba: nunca muestra 0:00 con tiempo por delante", () => {
    expect(formatRemaining(59_999)).toBe("1:00");
    expect(formatRemaining(1)).toBe("0:01");
  });

  test("0 y negativos son 0:00", () => {
    expect(formatRemaining(0)).toBe("0:00");
    expect(formatRemaining(-5)).toBe("0:00");
  });
});

describe("isWarning", () => {
  test("avisa en los últimos 20 segundos", () => {
    expect(isWarning(20_000)).toBe(true);
    expect(isWarning(20_001)).toBe(false);
    expect(isWarning(0)).toBe(true);
  });
});

describe("reasonText", () => {
  test("cada motivo de fin tiene un texto", () => {
    for (const reason of END_REASONS) {
      expect(reasonText(reason).length).toBeGreaterThan(0);
    }
  });

  test("los textos no repiten el nombre del celular ni datos del QR", () => {
    for (const reason of END_REASONS) {
      expect(reasonText(reason)).not.toContain("data:");
    }
  });
});
