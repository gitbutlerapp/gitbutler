import { safeCallback } from "$lib/auth/callback";
import { describe, expect, it } from "vitest";

describe("safeCallback", () => {
	const site = "https://gitbutler.com/login";

	it("keeps an address on this site or a local server", () => {
		expect(safeCallback("https://gitbutler.com/briefs/b/b123456789abc", site)).toBe(
			"https://gitbutler.com/briefs/b/b123456789abc",
		);
		expect(safeCallback("http://localhost:3000/briefs", site)).toBe("http://localhost:3000/briefs");
	});

	it("drops anything else", () => {
		for (const value of [
			null,
			"",
			"not a url",
			"/briefs",
			"javascript:alert(1)",
			"http://gitbutler.com/briefs",
			"https://gitbutler.com.evil.example/",
			"https://evil.example/?gitbutler.com",
		]) {
			expect(safeCallback(value, site)).toBeUndefined();
		}
	});
});
