import { pageLink } from "./site.ts";
import Markdown from "markdown-to-jsx";
import type { ComponentProps, FC } from "react";

/**
 * A design notes page: its Markdown file. Links between the files (`../foundations/motion.md`)
 * work on GitHub as written; here they open the file's page.
 */
export const DesignPage: FC<{ source: string }> = ({ source }) => (
	<div className="docs-site-prose">
		<Markdown options={{ overrides: { a: DesignLink } }}>{source}</Markdown>
	</div>
);

const DesignLink: FC<ComponentProps<"a">> = ({ href, children, ...props }) => {
	const page = /(?:^|\/)([a-z-]+)\/([a-z-]+)\.md$/.exec(href ?? "");
	return (
		<a {...props} {...(page ? pageLink(`design-${page.slice(1).join("-")}`) : { href })}>
			{children}
		</a>
	);
};
