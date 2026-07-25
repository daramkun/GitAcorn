import { describe, expect, it, vi } from "vitest";
import { resolveLocale, t } from "./i18n";

describe("i18n", () => {
  it("selects the first supported preferred language", () => {
    expect(resolveLocale(["ja-JP", "ko-KR", "en-US"])).toBe("ko");
    expect(resolveLocale(["en-US", "ko-KR"])).toBe("en");
  });

  it("falls back to English for unsupported languages", () => {
    expect(resolveLocale(["ja-JP"])).toBe("en");
  });

  it("translates and interpolates Korean text", () => {
    vi.stubGlobal("navigator", { languages: ["ko-KR"], language: "ko-KR" });
    expect(t("Move {name} left", { name: "GitAcorn" })).toBe(
      "GitAcorn 왼쪽으로 이동",
    );
    vi.unstubAllGlobals();
  });
});
