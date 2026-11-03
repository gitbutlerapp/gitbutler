import { Cell } from "../src/Cell.tsx";
import { Code } from "../src/Code.tsx";
import { RowGroup } from "../src/RowGroup.tsx";
import {
	isDesignPage,
	pageName,
	pages,
	type ComponentPageData,
	type Page,
	type StoryEntry,
} from "./pages.ts";
import { pageLink } from "./site.ts";
import type { FC } from "react";

/**
 * A component's page, drawn with the components themselves: its name and JSDoc, then each story
 * under its name, then the pages before and after it.
 */
export const ComponentPage: FC<{ page: Page; data: ComponentPageData }> = ({ page, data }) => {
	// An overlay's stories render open and a full view fills the window: each gets a frame of
	// its own, so it opens inside its card rather than over the page.
	const framed = (entry: StoryEntry) =>
		page.title.startsWith("Overlays/") || entry.story.parameters.layout === "fullscreen";

	return (
		<div className="docs-page">
			<header className="docs-page-header">
				<h1 className="docs-page-title">{pageName(page)}</h1>
				{data.description !== undefined && <Prose text={data.description} />}
			</header>
			{data.stories.map((entry) => (
				<Example key={entry.id} entry={entry} framed={framed(entry)} />
			))}
			<Neighbours page={page} />
		</div>
	);
};

const Example: FC<{ entry: StoryEntry; framed: boolean }> = ({ entry, framed }) => (
	<section className="docs-page-example">
		<h2 className="docs-page-example-title">{entry.name}</h2>
		{entry.description !== undefined && <Prose text={entry.description} />}
		<RowGroup>
			{framed ? (
				<iframe
					className="docs-page-frame"
					title={entry.name}
					src={`?story=${entry.id}`}
					loading="lazy"
				/>
			) : (
				<div className="docs-page-canvas">
					<entry.story.Component />
				</div>
			)}
		</RowGroup>
	</section>
);

const Neighbours: FC<{ page: Page }> = ({ page }) => {
	const components = pages.filter((other) => !isDesignPage(other));
	const at = components.indexOf(page);
	const previous = at > 0 ? components[at - 1] : undefined;
	const next = at + 1 < components.length ? components[at + 1] : undefined;

	return (
		<nav className="docs-page-neighbours" aria-label="Components">
			{previous === undefined ? (
				<div />
			) : (
				<PageLink page={previous} label="Previous" icon="arrow-left" />
			)}
			{next !== undefined && <PageLink page={next} label="Next" icon="arrow-right" />}
		</nav>
	);
};

const PageLink: FC<{
	page: Page;
	label: string;
	icon: "arrow-left" | "arrow-right";
}> = ({ page, label, icon }) => (
	<RowGroup>
		<Cell
			// oxlint-disable-next-line jsx_a11y/anchor-has-content -- The row renders as this link, and Cell fills it with the title.
			render={<a {...pageLink(page.id)} />}
			icon={icon}
			title={pageName(page)}
			description={label}
		/>
	</RowGroup>
);

/** JSDoc's paragraphs, its `code` and `{@link Name}` as code. */
const Prose: FC<{ text: string }> = ({ text }) =>
	text.split(/\n\s*\n/).map((paragraph) => (
		<p key={paragraph} className="docs-page-prose">
			{paragraph.split(/(`[^`]+`|\{@link [^}]+\})/).map((part, index) =>
				/^(`|\{@link )/.test(part) ? (
					// oxlint-disable-next-line react/no-array-index-key -- The parts of one paragraph don't move.
					<Code key={index}>{part.replace(/^`|`$|^\{@link |\}$/g, "")}</Code>
				) : (
					part
				),
			)}
		</p>
	));
