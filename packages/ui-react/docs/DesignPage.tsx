import { Markdown } from "../src/Markdown.tsx";
import { TextLink } from "../src/TextLink.tsx";
import { pageLink } from "./site.ts";
import type { FC, ReactNode } from "react";

/**
 * A design notes page: its Markdown file. Links between the files (`../foundations/motion.md`)
 * work on GitHub as written; here they open the file's page.
 */
export const DesignPage: FC<{ source: string }> = ({ source }) => (
	<div className="docs-site-prose">
		<Markdown renderLink={designLink}>{source}</Markdown>
	</div>
);

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
