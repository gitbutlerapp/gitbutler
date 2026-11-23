import { Cell } from "@gitbutler/but-ui-react/Cell.tsx";
import { Code } from "@gitbutler/but-ui-react/Code.tsx";
import { RowGroup } from "@gitbutler/but-ui-react/RowGroup.tsx";
import { DesignNote } from "./DesignPage.tsx";
import {
	isDesignPage,
	pageName,
	pages,
	type ComponentPageData,
	type Page,
	type StoryEntry,
} from "./pages.ts";
import { anchorOf, pageLink } from "./site.ts";
import { GpuiView } from "./GpuiView.tsx";
import { LinkedHeading } from "./Linked.tsx";
import { gpuiShots, type Renderer } from "./gpui.ts";
import { Icon } from "@gitbutler/but-ui-react/Icon.tsx";
import { Segment, SegmentedControl } from "@gitbutler/but-ui-react/SegmentedControl.tsx";
import { Tab, Tabs } from "@gitbutler/but-ui-react/Tabs.tsx";
import { useState, type CSSProperties, type FC, type ReactNode } from "react";

type PageTab = "specs" | "examples";

/** The tab the address names, `&tab=specs`, or Examples. */
const tabInAddress = (): PageTab =>
	new URLSearchParams(window.location.search).get("tab") === "specs" ? "specs" : "examples";

/**
 * A component's page, drawn with the components themselves: its name over two tabs, Examples
 * (each story under its name) and Specs (its JSDoc and the design notes its `@see` tags name),
 * then the pages before and after it. The tab picked goes in the address, so a link can open
 * either.
 */
export const ComponentPage: FC<{ page: Page; data: ComponentPageData }> = ({ page, data }) => {
	const [tab, setTab] = useState(tabInAddress);
	const pickTab = (next: PageTab) => {
		setTab(next);
		const address = new URL(window.location.href);
		if (next === "examples") address.searchParams.delete("tab");
		else address.searchParams.set("tab", next);
		window.history.replaceState(null, "", address);
	};
	// An overlay's stories render open and a full view fills the window: each gets a frame of
	// its own, so it opens inside its card rather than over the page.
	const framed = (entry: StoryEntry) =>
		page.title.startsWith("Overlays/") || entry.story.parameters.layout === "fullscreen";

	return (
		<div className="docs-page">
			<header className="docs-page-header">
				<h1 className="docs-page-title text-h1">{pageName(page)}</h1>
				<Tabs value={tab} onValueChange={pickTab} aria-label={pageName(page)}>
					<Tab value="examples" icon={<Icon name="slideshow" />} count={data.stories.length}>
						Examples
					</Tab>
					<Tab value="specs" icon={<Icon name="file-text" />}>
						Specs
					</Tab>
				</Tabs>
			</header>
			{tab === "specs" ? (
				<>
					{data.description !== undefined && (
						<div className="docs-page-description">
							<Prose text={data.description} />
						</div>
					)}
					{data.notes.map((note) => (
						<Guidelines key={note} source={note} named={data.notes.length > 1} />
					))}
				</>
			) : (
				data.stories.map((entry) => (
					<Example key={entry.id} entry={entry} framed={framed(entry)} gpui={page.gpui === true} />
				))
			)}
			<Neighbours page={page} />
		</div>
	);
};

/**
 * A design note under the component's description, headed Guidelines in its title's place, or
 * with its title after it when the page shows more than one.
 */
const Guidelines: FC<{ source: string; named: boolean }> = ({ source, named }) => {
	const title = /^# (.+)\n/m.exec(source)?.[1];
	const heading = named && title !== undefined ? `Guidelines: ${title}` : "Guidelines";
	return (
		<section className="docs-page-guidelines">
			<LinkedHeading id={anchorOf(heading)} className="docs-page-example-title text-h3">
				{heading}
			</LinkedHeading>
			<DesignNote source={source.replace(/^# .+\n/m, "")} folder="components" />
		</section>
	);
};

/**
 * A story under its name. On the page of a component the GPUI client draws too (`gpui`), its card
 * has a footer that switches it between the two: to the GPUI client's shot of the story, or, where
 * there is none yet, a GPUI that can't be picked, with a hint beside it saying so.
 */
const Example: FC<{ entry: StoryEntry; framed: boolean; gpui: boolean }> = ({
	entry,
	framed,
	gpui,
}) => {
	const { layout, width, height } = entry.story.parameters;
	const shots = gpuiShots(entry.id);
	const [renderer, pickRenderer] = useState<Renderer>("react");
	const showShot = shots !== undefined && renderer === "gpui";
	// Mounted from the start, hidden while React is picked, so the shot has loaded and holds its
	// height by the time it's picked rather than collapsing the card while it loads.
	const shot = shots !== undefined && (
		<GpuiView name={entry.name} shots={shots} hidden={!showShot} />
	);
	const footer = gpui && (
		<div className="docs-page-card-footer">
			<SegmentedControl value={renderer} onValueChange={pickRenderer} aria-label="Drawn by">
				<Segment value="react">React</Segment>
				<Segment value="gpui" disabled={shots === undefined}>
					GPUI
				</Segment>
			</SegmentedControl>
			{shots === undefined && (
				<span className="docs-page-card-hint text-12">Not drawn in GPUI yet</span>
			)}
		</div>
	);
	// A card is the story's size plus its border, centred on the column: a narrower one in it, a
	// wider one reaching past both its edges. A ground keeps to the prose line and only grows, past
	// both its edges, for a story wider than it.
	// A bar with no width of its own keeps to the prose line, as a ground does, so it shares the
	// words' edge.
	const card = width === undefined ? undefined : width + 2;
	const sized =
		card !== undefined
			? { width: card, alignSelf: "center" }
			: framed
				? undefined
				: { width: "100%", maxWidth: "var(--but-size-prose)", alignSelf: "center" };
	const ground =
		width === undefined
			? undefined
			: { width: `max(min(100%, var(--but-size-prose)), ${width + 48}px)`, maxWidth: "none" };
	return (
		<section className="docs-page-example">
			<LinkedHeading
				id={entry.id.split("--").at(-1) ?? anchorOf(entry.name)}
				className="docs-page-example-title text-h3"
			>
				{entry.name}
			</LinkedHeading>
			{entry.description !== undefined && <Prose text={entry.description} />}
			{framed || layout === "bar" ? (
				// A window or a bar the width of one: a bordered card, the window's edge.
				<RowGroup style={sized}>
					{shot}
					{framed ? (
						<iframe
							className="docs-page-frame"
							title={entry.name}
							src={`?story=${entry.id}`}
							loading="lazy"
							hidden={showShot}
							style={height === undefined ? undefined : { height }}
						/>
					) : (
						<div className="docs-page-canvas-bar" hidden={showShot}>
							<entry.story.Component />
						</div>
					)}
					{footer}
				</RowGroup>
			) : (
				<Ground entry={entry} style={ground} shot={shot} showShot={showShot} footer={footer} />
			)}
		</section>
	);
};

/**
 * Anything not framed stands on a tinted ground with no border, so a component with a card of its
 * own, as a RowGroup, isn't drawn as a card within a card. With a `footer`, the ground sits in a
 * card over it, and shows the GPUI client's `shot` in the story's place while that's picked
 * (`showShot`).
 */
const Ground: FC<{
	entry: StoryEntry;
	style: CSSProperties | undefined;
	shot: ReactNode;
	showShot: boolean;
	footer: ReactNode;
}> = ({ entry, style, shot, showShot, footer }) => {
	const { layout, width } = entry.story.parameters;
	const carded = footer !== false && footer !== undefined;
	const canvas = (
		<div className="docs-page-canvas" data-layout={layout} style={carded ? undefined : style}>
			{shot}
			<div
				className="docs-page-stage"
				hidden={showShot}
				style={width === undefined ? undefined : { width }}
			>
				<entry.story.Component />
			</div>
		</div>
	);
	if (!carded) return canvas;
	return (
		<div className="docs-page-card" data-layout={layout} style={style}>
			{canvas}
			{footer}
		</div>
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
