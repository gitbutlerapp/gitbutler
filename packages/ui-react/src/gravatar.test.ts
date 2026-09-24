import { describe, expect, test } from "vitest";
import { gravatarUrl, md5, withoutGeneratedFace } from "./gravatar.ts";

describe("md5", () => {
	test.each([
		["", "d41d8cd98f00b204e9800998ecf8427e"],
		["abc", "900150983cd24fb0d6963f7d28e17f72"],
		["The quick brown fox jumps over the lazy dog", "9e107d9d372bb6826bd81d3542a419d6"],
		// Longer than one 64-byte block, and non-ASCII.
		["a".repeat(100), "36a92cc94a9e0fa21f625f8bfb007adf"],
		["ümlaut", "6580044cd84290a2e56cf5a6605f5826"],
	])("%j", (input, hash) => {
		expect(md5(input)).toBe(hash);
	});
});

describe("gravatarUrl", () => {
	test("hashes an email as the backend does and asks for a real photo only", () => {
		expect(gravatarUrl(" Pavel@GitButler.com ", 18)).toBe(
			`https://www.gravatar.com/avatar/${md5("pavel@gitbutler.com")}?s=36&r=g&d=404`,
		);
	});
});

describe("withoutGeneratedFace", () => {
	test("turns off a Gravatar's generated face", () => {
		const url = new URL(
			withoutGeneratedFace("https://www.gravatar.com/avatar/abc?s=48&r=g&d=retro&f=y"),
		);
		expect(url.searchParams.get("d")).toBe("404");
		expect(url.searchParams.has("f")).toBe(false);
		expect(url.searchParams.get("s")).toBe("48");
	});

	test("replaces an initials image the server falls back to", () => {
		const url = new URL(
			withoutGeneratedFace(
				"https://s.gravatar.com/avatar/abc?s=480&r=pg&d=https%3A%2F%2Fcdn.auth0.com%2Favatars%2Fpa.png",
			),
		);
		expect(url.searchParams.get("d")).toBe("404");
	});

	test("leaves other pictures alone", () => {
		const url = "https://avatars.githubusercontent.com/u/1?v=4";
		expect(withoutGeneratedFace(url)).toBe(url);
	});
});
