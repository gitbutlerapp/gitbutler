import { codeContentToTokens } from "$lib/utils/diffParsing";
import { describe, expect, test } from "vitest";

describe.concurrent("codeContentToTokens", () => {
	test("returns one token array per line", () => {
		const result = codeContentToTokens("line1\nline2\nline3", undefined);
		expect(result).toHaveLength(3);
	});

	test("sanitizes HTML entities in plain text fallback", () => {
		const result = codeContentToTokens('<script>alert("xss")</script>', undefined);
		const html = result[0].join("");
		expect(html).not.toContain("<script>");
		expect(html).toContain("&lt;script&gt;");
		expect(html).toContain("&quot;");
	});

	test("sanitizes ampersands", () => {
		const result = codeContentToTokens("a && b", undefined);
		const html = result[0].join("");
		expect(html).toContain("&amp;&amp;");
	});

	test("sanitizes single quotes", () => {
		const result = codeContentToTokens("it's", undefined);
		const html = result[0].join("");
		expect(html).toContain("&#39;");
	});

	test("wraps content in span with data-no-drag", () => {
		const result = codeContentToTokens("hello", undefined);
		const html = result[0].join("");
		expect(html).toContain("data-no-drag");
	});
});
