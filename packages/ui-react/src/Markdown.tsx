import { getButtonClassName } from "./Button.tsx";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import { TextLink } from "./TextLink.tsx";
import { TooltipPopup } from "./Tooltip.tsx";
import { useCopied } from "./useCopied.ts";
import { Tooltip } from "@base-ui/react";
import type { CSSProperties, FC, MouseEvent, MouseEventHandler, ReactNode } from "react";
import { Suspense, use, useState } from "react";
import ReactMarkdown from "react-markdown";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import remarkGemoji from "remark-gemoji";
import remarkGfm from "remark-gfm";
import { codeToTokens, type BundledLanguage, type BundledTheme } from "shiki";
import styles from "./Markdown.module.css";

/** The links that leave the app — the only ones that open at all. */
const isExternalUrl = (url: string | undefined): url is string =>
	url !== undefined && (url.startsWith("http://") || url.startsWith("https://"));

/** How a link leaves; see {@link MarkdownProps.onOpenLink}. */
type OpenLink = MouseEventHandler<HTMLAnchorElement> | undefined;

/**
 * The anchor props that make a link leave the app: the host's handler when it
 * has one, otherwise a new tab. Image anchors may hold a source that isn't a
 * URL at all, and those the handler never sees.
 */
const leavingProps = (onOpenLink: OpenLink, href: string) =>
	onOpenLink === undefined
		? { target: "_blank", rel: "noreferrer" }
		: {
				onClick: (evt: MouseEvent<HTMLAnchorElement>) => {
					if (isExternalUrl(href)) onOpenLink(evt);
					else evt.preventDefault();
				},
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

const ImageLink: FC<{ src: string; alt: string; onOpenLink: OpenLink }> = ({
	src,
	alt,
	onOpenLink,
}) => (
	<a href={src} {...leavingProps(onOpenLink, src)} className={styles.imageLink}>
		<Icon name="paperclip" />
		{alt}
	</a>
);

/**
 * Inline image with a link fallback: private-repo attachments need browser
 * session cookies we don't have, so a failed load degrades to the link.
 */
const GitHubImage: FC<{ src: string; alt: string; onOpenLink: OpenLink }> = ({
	src,
	alt,
	onOpenLink,
}) => {
	const [failed, setFailed] = useState(false);

	if (failed) return <ImageLink src={src} alt={alt} onOpenLink={onOpenLink} />;

	return (
		<a href={src} {...leavingProps(onOpenLink, src)}>
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

/** The pair the diff viewer highlights with, for a host that says nothing. */
const defaultHighlightThemes = {
	light: "github-light-default",
	dark: "github-dark-default",
} as const satisfies HighlightThemes;

type Tokens = Awaited<ReturnType<typeof codeToTokens>>;

/**
 * One promise per block, so React's `use` sees the same one across renders
 * and a block re-rendering (its copy button ticking) doesn't re-tokenize.
 * Bounded like a cache should be; the oldest entries go first.
 */
const tokensCache = new Map<string, Promise<Tokens | null>>();
const TOKENS_CACHE_SIZE = 200;

const tokensFor = (
	code: string,
	language: string,
	themes: HighlightThemes,
): Promise<Tokens | null> => {
	const key = JSON.stringify([code, language, themes.light, themes.dark]);
	const cached = tokensCache.get(key);
	if (cached !== undefined) return cached;
	// Invalid names reject, and a rejection would reach an error boundary;
	// settled to null it keeps the plain block instead.
	const promise = codeToTokens(code, {
		lang: language as BundledLanguage,
		themes,
		defaultColor: false,
		cssVariablePrefix: "--shiki-",
	}).catch(() => null);
	tokensCache.set(key, promise);
	if (tokensCache.size > TOKENS_CACHE_SIZE) {
		const oldest = tokensCache.keys().next().value;
		if (oldest !== undefined) tokensCache.delete(oldest);
	}
	return promise;
};

/**
 * Fenced code with a language tag, highlighted through shiki's token API.
 * Tokens render as React spans — never HTML strings — so the no-innerHTML
 * property of this component is preserved. Colors come out as CSS variables
 * resolved with `light-dark()`, matching the app's theming. Unknown languages
 * reject, and the plain block stays; so does it while the tokens load.
 */
const CodeBlock: FC<{ language: string; code: string; themes: HighlightThemes }> = (props) => (
	<Suspense fallback={<code>{props.code}</code>}>
		<HighlightedCode {...props} />
	</Suspense>
);

const HighlightedCode: FC<{ language: string; code: string; themes: HighlightThemes }> = ({
	language,
	code,
	themes,
}) => {
	const tokensResult = use(tokensFor(code, language, themes));
	if (tokensResult === null) return <code>{code}</code>;

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

/** The syntax tree react-markdown hands each component, reduced to what reading text needs. */
type HastNode = {
	type: string;
	value?: string;
	children?: Array<HastNode>;
};

const hastText = (node: HastNode): string =>
	node.type === "text" ? (node.value ?? "") : (node.children ?? []).map(hastText).join("");

/**
 * A fenced code block with a button that copies its text. The button sits on
 * the block's corner rather than in the `<pre>`, which scrolls sideways and
 * would carry it away; the block's chrome moves out with it.
 */
const Pre: FC<{ node?: HastNode; children?: ReactNode; copyText: CopyText }> = ({
	node,
	children,
	copyText,
}) => {
	// A fence's text ends with the newline that closed it, which nobody wants pasted.
	const code = node === undefined ? "" : hastText(node).replace(/\n$/, "");
	const { copied, copy } = useCopied(code, copyText);

	return (
		<div className={styles.codeBlock}>
			<pre>{children}</pre>
			<Tooltip.Root>
				<Tooltip.Trigger
					className={classes(
						getButtonClassName({ variant: "ghost", size: "small", iconOnly: true }),
						styles.copy,
					)}
					// Keeps the button shown for the tick, even once the pointer has left the block.
					data-copied={copied || undefined}
					onClick={copy}
					render={<button type="button" aria-label={copied ? "Copied" : "Copy"} />}
				>
					{/* Each glyph in its own wrapper: the button styles the icons' opacity itself, so the
					    crossfade has to fade something else. */}
					<span className={styles.copyIcons}>
						<span className={classes(styles.copyIcon, copied && styles.copyIconGone)}>
							<Icon name="copy" />
						</span>
						<span className={classes(styles.copyIcon, !copied && styles.copyIconGone)}>
							<Icon name="tick" />
						</span>
					</span>
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup />}>{copied ? "Copied" : "Copy"}</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</div>
	);
};

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

/** A shiki theme per scheme: a bundled name, or another shiki knows by registration. */
export type HighlightThemes = {
	light: BundledTheme | (string & {});
	dark: BundledTheme | (string & {});
};

/** Puts `text` on the clipboard; see {@link MarkdownProps.copyText}. */
export type CopyText = ((text: string) => Promise<void>) | undefined;

export type MarkdownProps = {
	children: string;
	/**
	 * Opens a link that leaves: every http(s) link and image is one. Without
	 * it, the anchor opens a new tab, which is what a web page wants; Electron
	 * has to hand the URL to the system browser, as Lite does through its
	 * `openLinkExternally`.
	 */
	onOpenLink?: MouseEventHandler<HTMLAnchorElement>;
	/** Copies a code block's text; the clipboard API without it. */
	copyText?: (text: string) => Promise<void>;
	/** The shiki theme pair for fenced code; GitHub's light and dark defaults without it. */
	highlightThemes?: HighlightThemes;
};

/**
 * Renders forge-flavored markdown with GitHub-parity restrictions:
 *
 * - Raw HTML renders through {@link sanitizeSchema} — GitHub's safe subset,
 *   so `<details>` folds here like it does on github.com, while scripts,
 *   styles, event handlers, and unsafe URL schemes are stripped.
 * - Markdown-authored URLs additionally pass react-markdown's default
 *   transform; links only ever leave, through `onOpenLink`, and a host
 *   that can't have them navigate in place (Electron) blocks that itself.
 * - Images inline only from GitHub-operated hosts (which don't expose
 *   request logs to authors, so they can't track viewers); any other host
 *   renders as a link and is never fetched. See {@link isGitHubHostedImage}.
 * @import import { Markdown } from "@gitbutler/ui-react/Markdown.tsx";
 */
export const Markdown: FC<MarkdownProps> = ({
	children,
	onOpenLink,
	copyText,
	highlightThemes = defaultHighlightThemes,
}) => (
	<div className={classes("text-13", "text-body", styles.markdown)}>
		<ReactMarkdown
			remarkPlugins={[remarkGfm, remarkGemoji, remarkLiteralTags]}
			rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema]]}
			components={{
				a: ({ node: _node, children, href, ...props }) =>
					isExternalUrl(href) ? (
						<TextLink
							{...props}
							href={href}
							className={styles.externalLink}
							{...leavingProps(onOpenLink, href)}
						>
							{children}
						</TextLink>
					) : (
						<a {...props} href={href}>
							{children}
						</a>
					),
				code: ({ node: _node, className, children, ...props }) => {
					const language = fencedLanguage(className);
					return language !== undefined && typeof children === "string" ? (
						<CodeBlock
							language={language}
							code={children.replace(/\n$/, "")}
							themes={highlightThemes}
						/>
					) : (
						<code className={className} {...props}>
							{children}
						</code>
					);
				},
				pre: ({ node, children }) => (
					<Pre node={node} copyText={copyText}>
						{children}
					</Pre>
				),
				img: ({ node: _node, src, alt }) => {
					if (typeof src !== "string" || src === "") return null;
					const altText = typeof alt === "string" && alt !== "" ? alt : "image";
					return isGitHubHostedImage(src) ? (
						<GitHubImage src={src} alt={altText} onOpenLink={onOpenLink} />
					) : (
						<ImageLink src={src} alt={altText} onOpenLink={onOpenLink} />
					);
				},
			}}
		>
			{children}
		</ReactMarkdown>
	</div>
);
