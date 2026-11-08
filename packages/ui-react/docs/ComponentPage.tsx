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
				<h1 className="docs-page-title text-h1">{pageName(page)}</h1>
				{data.description !== undefined && <Prose text={data.description} />}
			</header>
			{data.stories.map((entry) => (
				<Example key={entry.id} entry={entry} framed={framed(entry)} />
			))}
			<Neighbours page={page} />
		</div>
	);
};

const Example: FC<{ entry: StoryEntry; framed: boolean }> = ({ entry, framed }) => {
	const { layout, width, height } = entry.story.parameters;
	// A card is the story's size plus its border, centred on the column: a narrower one in it, a
	// wider one reaching past both its edges. A ground keeps to the prose line and only grows, past
	// both its edges, for a story wider than it.
	const card = width === undefined ? undefined : width + 2;
	const sized = card === undefined ? undefined : { width: card, alignSelf: "center" };
	const ground =
		width === undefined
			? undefined
			: { width: `max(min(100%, var(--but-size-prose)), ${width + 48}px)`, maxWidth: "none" };
	return (
		<section className="docs-page-example">
			<h2 className="docs-page-example-title text-h3">{entry.name}</h2>
			{entry.description !== undefined && <Prose text={entry.description} />}
			{framed || layout === "bar" ? (
				// A window or a bar the width of one: a bordered card, the window's edge.
				<RowGroup style={sized}>
					{framed ? (
						<iframe
							className="docs-page-frame"
							title={entry.name}
							src={`?story=${entry.id}`}
							loading="lazy"
							style={height === undefined ? undefined : { height }}
						/>
					) : (
						<div className="docs-page-canvas-bar">
							<entry.story.Component />
						</div>
					)}
				</RowGroup>
			) : (
				// Anything else stands on a tinted ground with no border, so a component with a card of
				// its own, as a RowGroup, isn't drawn as a card within a card.
				<div className="docs-page-canvas" data-layout={layout} style={ground}>
					<div className="docs-page-stage" style={width === undefined ? undefined : { width }}>
						<entry.story.Component />
					</div>
				</div>
			)}
		</section>
	);
};

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
		<p key={paragraph} className="docs-page-prose text-prose">
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
