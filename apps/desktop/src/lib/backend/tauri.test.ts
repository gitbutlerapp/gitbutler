import { isValidDeepLinkUrl, parseDeepLinkUrl } from "$lib/backend/tauri";
import { describe, expect, test } from "vitest";

describe("isValidDeepLinkUrl", () => {
	test("returns true for valid but:// URLs", () => {
		expect(isValidDeepLinkUrl("but://open")).toBe(true);
		expect(isValidDeepLinkUrl("but://open?path=/some/path")).toBe(true);
		expect(isValidDeepLinkUrl("but://open/?path=C:\\some\\path")).toBe(true);
		expect(isValidDeepLinkUrl("but://open?path=/some/path&other=param")).toBe(true);
	});

	test("returns true for valid but-dev:// URLs", () => {
		expect(isValidDeepLinkUrl("but-dev://open")).toBe(true);
		expect(isValidDeepLinkUrl("but-dev://open?path=/dev/path")).toBe(true);
	});

	test("returns true for valid but-nightly:// URLs", () => {
		expect(isValidDeepLinkUrl("but-nightly://open")).toBe(true);
		expect(isValidDeepLinkUrl("but-nightly://open?path=/nightly/path")).toBe(true);
	});

	test("returns false for invalid schemes", () => {
		expect(isValidDeepLinkUrl("http://open")).toBe(false);
		expect(isValidDeepLinkUrl("https://open")).toBe(false);
		expect(isValidDeepLinkUrl("invalid://open")).toBe(false);
		expect(isValidDeepLinkUrl("but2://open")).toBe(false);
	});

	test("returns false for missing or invalid paths", () => {
		expect(isValidDeepLinkUrl("but://")).toBe(false);
		expect(isValidDeepLinkUrl("but://invalid")).toBe(false);
		expect(isValidDeepLinkUrl("but://invalid-path")).toBe(false);
		expect(isValidDeepLinkUrl("but://open-other")).toBe(false);
		expect(isValidDeepLinkUrl("but://open/extra?path=/some/path")).toBe(false);
		expect(isValidDeepLinkUrl("but://close")).toBe(false);
	});

	test("returns false for unsupported URL components", () => {
		expect(isValidDeepLinkUrl("but://user@open?path=/some/path")).toBe(false);
		expect(isValidDeepLinkUrl("but://open:123?path=/some/path")).toBe(false);
		expect(isValidDeepLinkUrl("but://open?path=/some/path#fragment")).toBe(false);
	});

	test("returns false for malformed URLs", () => {
		expect(isValidDeepLinkUrl("but:")).toBe(false);
		expect(isValidDeepLinkUrl("but:/")).toBe(false);
		expect(isValidDeepLinkUrl("but")).toBe(false);
		expect(isValidDeepLinkUrl("")).toBe(false);
	});
});

describe("parseDeepLinkUrl", () => {
	test("parses simple open URLs without query parameters", () => {
		const result = parseDeepLinkUrl("but://open" as any);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].toString()).toBe("");
	});

	test("parses open URLs with single query parameter", () => {
		const result = parseDeepLinkUrl("but://open?path=/some/path");
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("/some/path");
	});

	test("parses Windows-normalized URLs with a root path", () => {
		const result = parseDeepLinkUrl("but://open/?path=C:\\some\\path");
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("C:\\some\\path");
	});

	test("parses Windows-normalized login URLs", () => {
		const result = parseDeepLinkUrl("but://login/?access_token=token&t=123");
		expect(result).not.toBeNull();
		expect(result![0]).toBe("login");
		expect(result![1].get("access_token")).toBe("token");
	});

	test("parses open URLs with multiple query parameters", () => {
		const result = parseDeepLinkUrl("but://open?path=/some/path&other=value" as any);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("/some/path");
		expect(result![1].get("other")).toBe("value");
	});

	test("parses open URLs with new-window query parameter", () => {
		const result = parseDeepLinkUrl("but://open?path=/some/path&new_window=1" as any);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("/some/path");
		expect(result![1].get("new_window")).toBe("1");
	});

	test("handles URL-encoded query parameters", () => {
		const result = parseDeepLinkUrl("but://open?path=%2Fsome%2Fpath" as any);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("/some/path");
	});

	test("handles query parameters with special characters", () => {
		const result = parseDeepLinkUrl(
			"but://open?path=/path/with%20spaces&key=value%26special" as any,
		);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("/path/with spaces");
		expect(result![1].get("key")).toBe("value&special");
	});

	test("works with all valid schemes", () => {
		const schemes = ["but", "but-dev", "but-nightly"];
		schemes.forEach((scheme) => {
			const result = parseDeepLinkUrl(`${scheme}://open?path=/test` as any);
			expect(result).not.toBeNull();
			expect(result![0]).toBe("open");
			expect(result![1].get("path")).toBe("/test");
		});
	});

	test("returns null for missing path part", () => {
		const result = parseDeepLinkUrl("but://" as any);
		expect(result).toBeNull();
	});

	test("returns null for invalid top-level path", () => {
		const result = parseDeepLinkUrl("but://invalid" as any);
		expect(result).toBeNull();
	});

	test("returns null for empty path", () => {
		const result = parseDeepLinkUrl("but://?path=/test" as any);
		expect(result).toBeNull();
	});

	test("handles URLs with empty query string", () => {
		const result = parseDeepLinkUrl("but://open?" as any);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].toString()).toBe("");
	});

	test("handles URLs with trailing characters after query", () => {
		const result = parseDeepLinkUrl("but://open?path=/test&" as any);
		expect(result).not.toBeNull();
		expect(result![0]).toBe("open");
		expect(result![1].get("path")).toBe("/test");
	});
});
