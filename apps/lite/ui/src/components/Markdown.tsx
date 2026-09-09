import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { useQuery } from "@tanstack/react-query";
import type { CSSProperties, FC, MouseEvent } from "react";
import { useState } from "react";
import ReactMarkdown from "react-markdown";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import remarkGemoji from "remark-gemoji";
import remarkGfm from "remark-gfm";
import { codeToTokens, type BundledLanguage } from "shiki";
import styles from "./Markdown.module.css";

/** The links that leave the app — the only ones that open at all. */
const isExternalUrl = (url: string | undefined): url is string =>
	url !== undefined && (url.startsWith("http://") || url.startsWith("https://"));

const openExternally = (evt: MouseEvent<HTMLAnchorElement>): void => {
	evt.preventDefault();
	const url = evt.currentTarget.href;
	if (isExternalUrl(url)) {
		window.lite.openInWebBrowser(url).catch((error: unknown) => {
			// oxlint-disable-next-line no-console
			console.error(error);
		});
	}
};

/**
 * GitHub-operated image hosts; this is the UX decision and the CSP's
 * `img-src` is the enforcement (it additionally allows GitHub's signed S3
 * bucket, the redirect *target* of user-attachments URLs — sources are
 * still only ever these hosts). GitHub launders third-party images through
 * camo only in its own rendered HTML — raw markdown keeps the original
 * URL — so external hosts stay links and can't track viewers, matching
 * github.com's own privacy posture.
 */
const isGitHubHostedImage = (src: string): boolean => {
	try {
		const url = new URL(src);
		return (
			url.protocol === "https:" &&
			(url.hostname === "github.com" || url.hostname.endsWith(".githubusercontent.com"))
		);
	} catch {
		return false;
	}
};

const ImageLink: FC<{ src: string; alt: string }> = ({ src, alt }) => (
	<a href={src} onClick={openExternally} className={styles.imageLink}>
		<Icon name="paperclip" />
		{alt}
	</a>
);

/**
 * Inline image with a link fallback: private-repo attachments need browser
 * session cookies we don't have, so a failed load degrades to the link.
 */
const GitHubImage: FC<{ src: string; alt: string }> = ({ src, alt }) => {
	const [failed, setFailed] = useState(false);

	if (failed) return <ImageLink src={src} alt={alt} />;

	return (
		<a href={src} onClick={openExternally}>
			<img
				src={src}
				alt={alt}
				loading="lazy"
				className={styles.image}
				onError={() => setFailed(true)}
			/>
		</a>
	);
};

/**
 * GitHub-parity sanitization: rehype-sanitize's default schema is modeled on
 * GitHub's own pipeline (safe tag subset incl. `details`/`summary`/`kbd`, no
 * `style` attributes, no event handlers, `javascript:`/`data:` URLs dropped).
 * The one addition: strip `style` elements entirely — the default unwraps
 * them, which would leak the CSS source as visible text.
 */
const sanitizeSchema = {
	...defaultSchema,
	strip: [...(defaultSchema.strip ?? []), "style"],
};

/**
 * Fenced code with a language tag, highlighted through shiki's token API.
 * Tokens render as React spans — never HTML strings — so the no-innerHTML
 * property of this component is preserved. Colors come out as CSS variables
 * resolved with `light-dark()`, matching the app's theming, using the same
 * theme pair as the diff viewer. Unknown languages fall back to plain text.
 */
const CodeBlock: FC<{ language: string; code: string }> = ({ language, code }) => {
	const { data: themeCfg } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.syntaxHighlighting,
	});
	const light = themeCfg?.light ?? defaultSettings.syntaxHighlighting.light;
	const dark = themeCfg?.dark ?? defaultSettings.syntaxHighlighting.dark;

	const { data: tokensResult } = useQuery({
		queryKey: ["markdownTokens", code, language, light, dark],
		queryFn: () =>
			codeToTokens(code, {
				// Invalid names reject and we keep the plain fallback.
				lang: language as BundledLanguage,
				themes: { light, dark },
				defaultColor: false,
				cssVariablePrefix: "--shiki-",
			}),
	});

	if (tokensResult === undefined) return <code>{code}</code>;

	return (
		<code className={styles.highlighted}>
			{tokensResult.tokens.map((line, lineIdx) => (
				// Lines are positional; there is no stable identity to key on.
				// oxlint-disable-next-line react/no-array-index-key
				<span key={lineIdx}>
					{line.map((token, tokenIdx) => (
						// oxlint-disable-next-line react/no-array-index-key
						<span key={tokenIdx} style={token.htmlStyle as CSSProperties | undefined}>
							{token.content}
						</span>
					))}
					{"\n"}
				</span>
			))}
		</code>
	);
};

const fencedLanguage = (className: string | undefined): string | undefined =>
	/language-([\w+#-]+)/.exec(className ?? "")?.[1];

type MarkdownNode = {
	type: string;
	value?: string;
	children?: Array<MarkdownNode>;
};

// Review prose sometimes mentions a tag without backticks. Preserve a lone
// tag inside a paragraph before the HTML parser can split the sentence around it.
const remarkLiteralTags = () => {
	const visit = (node: MarkdownNode): void => {
		if (["paragraph", "emphasis", "strong", "delete", "link"].includes(node.type)) {
			const html = node.children?.filter((child) => child.type === "html") ?? [];
			const markup = html.map((child) => child.value).join("");
			for (const child of html) {
				const match = /^<(\/?)([a-z][a-z0-9-]*)>$/i.exec(child.value ?? "");
				if (!match) continue;
				const [, closing, tag] = match;
				if (
					tag === undefined ||
					/^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(tag)
				)
					continue;

				const counterpart = new RegExp(
					closing === "/" ? `<${tag}(?:\\s[^>]*|)>` : `</${tag}\\s*>`,
					"i",
				);
				if (!counterpart.test(markup)) child.type = "inlineCode";
			}
		}
		for (const child of node.children ?? []) visit(child);
	};
	return visit;
};

/**
 * Renders forge-flavored markdown with GitHub-parity restrictions:
 *
 * - Raw HTML renders through {@link sanitizeSchema} — GitHub's safe subset,
 *   so `<details>` folds here like it does on github.com, while scripts,
 *   styles, event handlers, and unsafe URL schemes are stripped.
 * - Markdown-authored URLs additionally pass react-markdown's default
 *   transform; links only open via the system browser, and the Electron
 *   shell blocks all in-app navigation.
 * - Images inline only from GitHub-operated hosts (which don't expose
 *   request logs to authors, so they can't track viewers); any other host
 *   renders as a link and is never fetched. See {@link isGitHubHostedImage}.
 */
export const Markdown: FC<{ children: string }> = ({ children }) => (
	<div className={classes("text-13", "text-body", styles.markdown)}>
		<ReactMarkdown
			remarkPlugins={[remarkGfm, remarkGemoji, remarkLiteralTags]}
			rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema]]}
			components={{
				a: ({ node: _node, children, ...props }) => (
					// oxlint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- href arrives via the spread; it stays a real anchor.
					<a {...props} onClick={openExternally}>
						{children}
						{isExternalUrl(props.href) && (
							<Icon name="arrow-up-right" size={12} className={styles.externalIcon} />
						)}
					</a>
				),
				code: ({ node: _node, className, children, ...props }) => {
					const language = fencedLanguage(className);
					return language !== undefined && typeof children === "string" ? (
						<CodeBlock language={language} code={children.replace(/\n$/, "")} />
					) : (
						<code className={className} {...props}>
							{children}
						</code>
					);
				},
				img: ({ node: _node, src, alt }) => {
					if (typeof src !== "string" || src === "") return null;
					const altText = typeof alt === "string" && alt !== "" ? alt : "image";
					return isGitHubHostedImage(src) ? (
						<GitHubImage src={src} alt={altText} />
					) : (
						<ImageLink src={src} alt={altText} />
					);
				},
			}}
		>
			{children}
		</ReactMarkdown>
	</div>
);
