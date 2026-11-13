import tokens from "../../but-ui/tokens/tokens.json";
import { Markdown } from "../src/Markdown.tsx";
import { TextLink } from "../src/TextLink.tsx";
import { notePage } from "./pages.ts";
import { pageLink } from "./site.ts";
import type { FC, ReactNode } from "react";

/**
 * A design notes page: its Markdown file. Links between the files (`../foundations/motion.md`,
 * or `buttons.md` beside it) work on GitHub as written; here they open the file's page. A note
 * names a token, never its value, so the page puts today's value after it.
 */
export const DesignPage: FC<{ source: string; folder: string }> = ({ source, folder }) => (
	<div className="docs-site-prose">
		<DesignNote source={source} folder={folder} />
	</div>
);

/** A design note's Markdown, on a page of its own or on its component's; `folder` is the one it
 * is in, where a link to a file beside it points. */
export const DesignNote: FC<{ source: string; folder: string }> = ({ source, folder }) => (
	<Markdown renderLink={(href, children) => designLink(folder, href, children)}>
		{withValues(source)}
	</Markdown>
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

/** A component's note opens the component's page, which shows it; any other note its own page. */
const designLink = (from: string, href: string, children: ReactNode) => {
	const [, folder = from, name] = /^(?:(?:.*\/)?([a-z-]+)\/)?([a-z-]+)\.md$/.exec(href) ?? [];
	const id =
		name === undefined
			? undefined
			: folder === "components"
				? notePage(`../but-ui/design/components/${name}.md`)?.id
				: `design-${folder}-${name}`;
	return id !== undefined ? (
		<a {...pageLink(id)} className="docs-site-page-link">
			{children}
		</a>
	) : (
		<TextLink href={href} target="_blank" rel="noreferrer">
			{children}
		</TextLink>
	);
};
