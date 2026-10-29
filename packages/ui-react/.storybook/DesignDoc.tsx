import { Markdown } from "@storybook/addon-docs/blocks";
import type { ComponentProps } from "react";

/**
 * A design notes page in Storybook: its Markdown file, less the heading, which
 * the MDX page writes itself so the MCP server's `docs-show` (which returns the
 * MDX source) has the title and summary. Links between the files
 * (`../foundations/motion.md`) work on GitHub as written; here they open the
 * page's Storybook entry, whose title follows the same group and name.
 */
export function DesignDoc({ source }: { source: string }) {
	return (
		<Markdown options={{ overrides: { code: "code", a: DesignDocLink } }}>
			{source.replace(/^# .*\n+/, "")}
		</Markdown>
	);
}

function DesignDocLink({ href, children, ...props }: ComponentProps<"a">) {
	const page = href?.match(/(?:^|\/)([a-z-]+)\/([a-z-]+)\.md$/);
	if (!page) {
		return (
			<a href={href} {...props}>
				{children}
			</a>
		);
	}
	// The docs render in the preview iframe; the sidebar's URL is the top window's.
	return (
		<a href={`./?path=/docs/design-${page[1]}-${page[2]}--docs`} target="_top" {...props}>
			{children}
		</a>
	);
}
