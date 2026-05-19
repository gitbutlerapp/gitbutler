import { applyThemeToDocument, getResolvedThemeId, THEME_OPTIONS } from "$lib/theme/themes";
import { describe, expect, test } from "vitest";

describe("theme registry", () => {
	test("offers a large palette set", () => {
		expect(THEME_OPTIONS.length).toBeGreaterThanOrEqual(10);
	});

	test("resolves system preference to the active color scheme", () => {
		expect(getResolvedThemeId("system", "dark")).toBe("dark");
		expect(getResolvedThemeId(undefined, "light")).toBe("light");
	});

	test("applies custom theme variables to the document root", () => {
		const docEl = document.createElement("html");

		applyThemeToDocument(docEl, "poimandres", "light");

		expect(docEl.dataset.theme).toBe("poimandres");
		expect(docEl.dataset.resolvedTheme).toBe("poimandres");
		expect(docEl.classList.contains("dark")).toBe(true);
		expect(docEl.style.getPropertyValue("--clr-pop-50")).toContain("#5de4c7");
	});

	test("clears custom variables when returning to the built-in themes", () => {
		const docEl = document.createElement("html");

		applyThemeToDocument(docEl, "poimandres", "light");
		applyThemeToDocument(docEl, "system", "light");

		expect(docEl.dataset.resolvedTheme).toBe("light");
		expect(docEl.style.getPropertyValue("--clr-pop-50")).toBe("");
	});
});
