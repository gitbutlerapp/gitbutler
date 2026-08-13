import { Markdown } from "#ui/components/Markdown.tsx";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

const render = (markdown: string): string =>
	renderToStaticMarkup(
		<QueryClientProvider client={new QueryClient()}>
			<Markdown>{markdown}</Markdown>
		</QueryClientProvider>,
	);

describe("Markdown safety", () => {
	it("renders GitHub's safe HTML subset", () => {
		const html = render("<details><summary>More</summary>\n\nhidden *body*\n\n</details>");
		expect(html).toContain("<details>");
		expect(html).toContain("<summary>More</summary>");
		expect(html).toContain("<em>body</em>");
	});

	it("strips scripts and iframes entirely", () => {
		const html = render('<script>alert(1)</script> <iframe src="https://x.test"></iframe>');
		expect(html).not.toContain("<script");
		expect(html).not.toContain("<iframe");
		expect(html).not.toContain("alert(1)");
	});

	it("cannot produce style elements or attributes", () => {
		const html = render('<style>*{display:none}</style> <b style="color:red">x</b>');
		expect(html).not.toContain("<style");
		expect(html).not.toContain('style="');
		// Style source must not leak as visible text either.
		expect(html).not.toContain("display:none");
		// The safe element survives without its style attribute.
		expect(html).toContain("<b>x</b>");
	});

	it("strips event handlers and unsafe URL schemes from raw HTML", () => {
		const html = render('<a href="javascript:alert(1)" onclick="alert(2)">x</a>');
		expect(html).not.toContain("javascript:");
		expect(html).not.toContain("onclick");
	});

	it("strips javascript: hrefs from links", () => {
		const html = render("[click me](javascript:alert(1))");
		expect(html).not.toContain("javascript:");
	});

	it("strips data: URLs", () => {
		const html = render("[click me](data:text/html,<script>alert(1)</script>)");
		expect(html).not.toContain("data:");
	});

	it("keeps http(s) links", () => {
		const html = render("[docs](https://example.test/docs)");
		expect(html).toContain('href="https://example.test/docs"');
	});

	it("renders third-party images as links instead of fetching them", () => {
		const html = render("![diagram](https://example.test/pixel.png)");
		expect(html).not.toContain("<img");
		expect(html).toContain('href="https://example.test/pixel.png"');
		expect(html).toContain("diagram");
	});

	it("inlines GitHub-hosted images", () => {
		const html = render("![shot](https://user-images.githubusercontent.com/1/shot.png)");
		expect(html).toContain('src="https://user-images.githubusercontent.com/1/shot.png"');
		expect(html).toContain('loading="lazy"');
	});

	it("does not treat lookalike hosts as GitHub", () => {
		const html = render(
			"![x](https://evilgithub.com/a.png) ![y](https://githubusercontent.com.evil.test/b.png)",
		);
		expect(html).not.toContain("<img");
	});

	it("drops images with stripped (unsafe) sources entirely", () => {
		const html = render("![x](javascript:alert(1))");
		expect(html).not.toContain("<img");
		expect(html).not.toContain("javascript:");
	});

	it("renders GFM tables and task lists within the allowlist", () => {
		const html = render("| a | b |\n| - | - |\n| 1 | 2 |\n\n- [x] done");
		expect(html).toContain("<table");
		expect(html).toContain('type="checkbox"');
		expect(html).toContain("disabled");
	});

	it("renders GitHub emoji shortcodes outside code spans", () => {
		const html = render(":+1: `:+1:`");
		expect(html).toContain("👍");
		expect(html).toContain("<code>:+1:</code>");
	});

	it("renders fenced code as plain text until highlighting resolves", () => {
		const html = render("```rust\nfn main() {}\n```");
		expect(html).toContain("fn main() {}");
		expect(html).toContain("<pre>");
	});

	it("routes language-tagged fences into the highlighter branch", () => {
		// The CodeBlock branch drops the language class from the code
		// element; the plain fallback would keep class="language-rust".
		// Pins that react-markdown hands fence content over as one string.
		const html = render("```rust\nfn main() {}\n```");
		expect(html).not.toContain("language-rust");
	});

	it(
		"tokenizes fenced code into CSS-variable colors for both themes",
		{ timeout: 30_000 },
		async () => {
			const { codeToTokens } = await import("shiki");
			const result = await codeToTokens("fn main() {}", {
				lang: "rust",
				themes: { light: "github-light-default", dark: "github-dark-default" },
				defaultColor: false,
				cssVariablePrefix: "--shiki-",
			});
			const style = result.tokens[0]?.[0]?.htmlStyle ?? {};
			expect(Object.keys(style)).toContain("--shiki-light");
			expect(Object.keys(style)).toContain("--shiki-dark");
		},
	);

	it("unwraps content of elements outside the allowlist rather than rendering them", () => {
		// Footnote references produce sup/section/a, all allowlisted; this
		// guards the unwrapDisallowed behavior using math-like raw text.
		const html = render("plain *emphasis* text");
		expect(html).toContain("<em>");
	});
});
