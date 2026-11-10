import { FieldControlWithIcon } from "../src/Field.tsx";
import { PopupSearch } from "../src/Popup.tsx";
import { ResizeHandle } from "../src/ResizeHandle.tsx";
import { ScrollArea } from "../src/ScrollArea.tsx";
import { Segment, SegmentedControl } from "../src/SegmentedControl.tsx";
import { GroupLabel, NavItem } from "../src/SidebarRow.tsx";
import { ComponentPage } from "./ComponentPage.tsx";
import { DesignPage } from "./DesignPage.tsx";
import type { IconName } from "../src/iconNames.ts";
import {
	findPage,
	groupPages,
	isDesignPage,
	loadPage,
	pageName,
	pages,
	type Page,
} from "./pages.ts";
import {
	accentOf,
	currentPage,
	pageLink,
	pickAccent,
	pickRadius,
	pickTheme,
	savedAccent,
	savedRadius,
	savedTheme,
	subscribePage,
	type Accent,
	type Radius,
	type Theme,
} from "./site.ts";
import { Suspense, use, useState, useSyncExternalStore, type FC } from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";

/**
 * The docs site: the search, the pages in their cards, the theme, and the page in a card of its own.
 * The sidebar is as wide as it was last dragged.
 */
export const Site: FC = () => {
	const page = useSyncExternalStore(subscribePage, currentPage);
	const layout = useDefaultLayout({ id: "docs-site", storage: localStorage });

	return (
		<Group orientation="horizontal" className="docs-site" style={{ height: "100vh" }} {...layout}>
			<Panel id="sidebar" defaultSize="280px" minSize="220px" maxSize="400px">
				<Navigation current={page} />
			</Panel>
			<ResizeHandle gap className="docs-site-handle" />
			<Panel id="page">
				<main className="docs-site-main">
					<Suspense>{page !== undefined && <PageView key={page.id} page={page} />}</Suspense>
				</main>
			</Panel>
		</Group>
	);
};

const PageView: FC<{ page: Page }> = ({ page }) => {
	const data = use(loadPage(page));
	return data.kind === "design" ? (
		<DesignPage source={data.source} />
	) : (
		<ComponentPage page={page} data={data} />
	);
};

/** The sidebar's cards: the design notes under Docs, which opens the overview, and the components'
 * stories under Components. */
const sections: Array<{
	name: string;
	icon: IconName;
	page?: Page;
	holds: (page: Page) => boolean;
}> = [
	{ name: "Docs", icon: "docs", page: findPage("design-overview"), holds: isDesignPage },
	{ name: "Components", icon: "stack", holds: (page) => !isDesignPage(page) },
];

const Navigation: FC<{ current: Page | undefined }> = ({ current }) => {
	const [query, setQuery] = useState("");
	const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());
	const found = pages.filter((page) =>
		page.title.toLowerCase().includes(query.trim().toLowerCase()),
	);
	const toggle = (key: string) =>
		setFolded((previous) => {
			const next = new Set(previous);
			if (!next.delete(key)) next.add(key);
			return next;
		});

	return (
		<aside className="docs-site-sidebar">
			<div className="docs-site-card docs-site-search">
				<PopupSearch
					placeholder="Filter pages"
					aria-label="Filter the pages"
					value={query}
					onChange={(event) => setQuery(event.target.value)}
					onClear={query === "" ? undefined : () => setQuery("")}
				/>
			</div>
			<ScrollArea className="docs-site-scroll" viewportClassName="docs-site-scroll-viewport">
				{/* The page shown is the selection, so it takes the solid fill of a focused one. */}
				<nav aria-label="Pages" data-selection-focused="true">
					{sections.map((section) => {
						const held = found.filter((page) => section.holds(page) && page !== section.page);
						if (held.length === 0 && (section.page === undefined || !found.includes(section.page)))
							return null;
						// Keyed apart from the groups: Docs has a Components folder of its own.
						const key = `section:${section.name}`;
						return (
							<section
								key={section.name}
								className="docs-site-section"
								aria-label={section.name}
								data-folded={folded.has(key) ? "" : undefined}
							>
								<div className="docs-site-section-head">
									<div className="docs-site-section-top">
										<NavItem
											leadingRow
											name={section.name}
											icon={section.icon}
											folded={folded.has(key)}
											onToggleFolded={() => toggle(key)}
											selected={section.page !== undefined && section.page === current}
											render={
												section.page === undefined ? undefined : (
													// oxlint-disable-next-line jsx_a11y/anchor-has-content -- The row renders as this link, and NavItem fills it with the name.
													<a
														{...pageLink(section.page.id)}
														aria-current={section.page === current ? "page" : undefined}
													/>
												)
											}
										/>
									</div>
								</div>
								{!folded.has(key) &&
									groupPages(held).map(([heading, group]) => (
										<div key={heading} className="docs-site-group">
											<GroupLabel
												name={heading}
												folded={folded.has(heading)}
												onToggleFolded={() => toggle(heading)}
											/>
											{!folded.has(heading) &&
												group.map((page) => (
													<NavItem
														key={page.id}
														name={pageName(page)}
														depth={1}
														selected={page === current}
														render={
															// oxlint-disable-next-line jsx_a11y/anchor-has-content -- The row renders as this link, and NavItem fills it with the name.
															<a
																{...pageLink(page.id)}
																aria-current={page === current ? "page" : undefined}
															/>
														}
													/>
												))}
										</div>
									))}
								{!folded.has(key) && <div className="docs-site-section-foot" aria-hidden />}
							</section>
						);
					})}
				</nav>
			</ScrollArea>
			<div className="docs-site-appearance">
				<ThemeSwitch />
				<RadiusSwitch />
				<AccentPicker />
			</div>
		</aside>
	);
};

/** Teal, blue and purple are the ramps the tokens name; amber and red, like anything typed, are
 * a colour the tokens draw at teal's lightness. */
const ACCENTS: Array<{ name: string; accent: Accent; hex: string }> = [
	{ name: "Teal", accent: "teal", hex: "#008383" },
	{ name: "Blue", accent: "blue", hex: "#2571E3" },
	{ name: "Purple", accent: "purple", hex: "#8943D0" },
	{ name: "Amber", accent: "#FF9500", hex: "#FF9500" },
	{ name: "Red", accent: "#D5412A", hex: "#D5412A" },
];

const hexOf = (accent: Accent) => ACCENTS.find((a) => a.accent === accent)?.hex ?? accent;

const AccentPicker: FC = () => {
	const [accent, setAccent] = useState(savedAccent);
	const [draft, setDraft] = useState(() => hexOf(accent));
	const pick = (next: Accent) => {
		setAccent(next);
		setDraft(hexOf(next));
		pickAccent(next);
	};
	const type = (text: string) => {
		setDraft(text);
		const typed = accentOf(text.trim());
		if (typed === "teal") return;
		const next = ACCENTS.find((a) => a.hex === typed)?.accent ?? typed;
		setAccent(next);
		pickAccent(next);
	};

	return (
		<div className="docs-site-accent">
			<div role="radiogroup" aria-label="Accent" className="docs-site-swatches">
				{ACCENTS.map(({ name, accent: value, hex }) => (
					<input
						key={name}
						type="radio"
						name="docs-accent"
						checked={value === accent}
						aria-label={name}
						title={name}
						className="docs-site-swatch"
						style={{ color: hex }}
						onChange={() => pick(value)}
					/>
				))}
			</div>
			<FieldControlWithIcon
				icon={<span className="docs-site-swatch-dot" />}
				aria-label="Accent colour"
				spellCheck={false}
				value={draft}
				onChange={(event) => type(event.target.value)}
				onBlur={() => setDraft(hexOf(accent))}
			/>
		</div>
	);
};

const RadiusSwitch: FC = () => {
	const [radius, setRadius] = useState(savedRadius);
	const pick = (next: Radius) => {
		setRadius(next);
		pickRadius(next);
	};

	return (
		<SegmentedControl fill value={radius} onValueChange={pick} aria-label="Corners">
			<Segment value="sharp">Sharp</Segment>
			<Segment value="default">Default</Segment>
			<Segment value="round">Round</Segment>
		</SegmentedControl>
	);
};

const ThemeSwitch: FC = () => {
	const [theme, setTheme] = useState(savedTheme);
	const pick = (next: Theme) => {
		setTheme(next);
		pickTheme(next);
	};

	return (
		<SegmentedControl fill value={theme} onValueChange={pick} aria-label="Theme">
			<Segment value="system">System</Segment>
			<Segment value="light">Light</Segment>
			<Segment value="dark">Dark</Segment>
		</SegmentedControl>
	);
};
