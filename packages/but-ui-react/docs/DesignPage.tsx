import tokens from "../../but-ui/tokens/tokens.json";
import { Markdown } from "../src/Markdown.tsx";
import { TextLink } from "../src/TextLink.tsx";
import { pageLink } from "./site.ts";
import type { FC, ReactNode } from "react";

/**
 * A design notes page: its Markdown file. Links between the files (`../foundations/motion.md`)
 * work on GitHub as written; here they open the file's page. A note names a token, never its
 * value, so the page puts today's value after it.
 */
export const DesignPage: FC<{ source: string }> = ({ source }) => (
	<div className="docs-site-prose">
		<Markdown renderLink={designLink}>{withValues(source)}</Markdown>
	</div>
);

/** Each length and duration token's value as Figma has it, by its name without `--but-`. */
const values = new Map<string, string>([
	...Object.entries(tokens.lengths).map(([name, px]): [string, string] => [name, `${px}px`]),
	...Object.entries(tokens.durations).map(([name, ms]): [string, string] => [name, `${ms}ms`]),
]);

/** `source` with each token it names in code followed by its value, as `--but-transition-fast` (80ms). */
function withValues(source: string): string {
	return source.replaceAll(/`--but-([a-z0-9-]+)`/g, (code, name: string) => {
		const value = values.get(name);
		return value === undefined ? code : `${code} (${value})`;
	});
}

const designLink = (href: string, children: ReactNode) => {
	const page = /(?:^|\/)([a-z-]+)\/([a-z-]+)\.md$/.exec(href);
	return page ? (
		<a {...pageLink(`design-${page.slice(1).join("-")}`)} className="docs-site-page-link">
			{children}
		</a>
	) : (
		<TextLink href={href} target="_blank" rel="noreferrer">
			{children}
		</TextLink>
	);
};
