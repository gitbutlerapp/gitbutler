import { Menu, MenuRadioGroup, MenuRadioItem } from "../src/Menu.tsx";
import { PopupSearch } from "../src/Popup.tsx";
import { ResizeHandle } from "../src/ResizeHandle.tsx";
import { ScrollArea } from "../src/ScrollArea.tsx";
import { GroupLabel, NavItem } from "../src/SidebarRow.tsx";
import { treeGuides } from "../src/treeGuides.ts";
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
	currentPage,
	pageLink,
	PRESETS,
	presetOf,
	pickAccent,
	pickGround,
	pickDensity,
	pickPreset,
	pickRadius,
	pickTheme,
	savedAccent,
	savedGround,
	savedDensity,
	savedRadius,
	savedTheme,
	subscribePage,
	type Accent,
	type Ground,
	type Density,
	type Radius,
	type Theme,
} from "./site.ts";
import {
	Suspense,
	use,
	useState,
	useSyncExternalStore,
	type FC,
	type ReactElement,
	type ReactNode,
} from "react";
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
	{ name: "Components", icon: "components", holds: (page) => !isDesignPage(page) },
];

/** Each group's glyph, by its heading: the design notes' folders and the components' shelves. */
const GROUP_ICONS: Record<string, IconName> = {
	"Getting started": "play",
	Foundations: "4x4-grid",
	Content: "text-plain",
	Components: "components",
	Patterns: "hash",
	Actions: "pointer",
	Inputs: "keyboard",
	Overlays: "stack",
	"Content & status": "tag",
	"Files & diffs": "file-diff",
	Illustrations: "image",
	Preview: "layout",
	Sidebar: "sidebar",
	"Workspace tree": "folder-tree",
	Icons: "smiley",
};

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
												icon={GROUP_ICONS[heading]}
												folded={folded.has(heading)}
												onToggleFolded={() => toggle(heading)}
											/>
											{!folded.has(heading) &&
												group.map((page, index) => (
													<NavItem
														key={page.id}
														name={pageName(page)}
														depth={1}
														guides={pageGuides(group.length)[index]}
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
			<Appearance />
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

/** Gray, Figma's, and the tinted grays, each drawn by its own step 9. */
const GROUNDS: Record<Ground, { name: string }> = {
	gray: { name: "Gray" },
	slate: { name: "Slate" },
	sage: { name: "Sage" },
	sand: { name: "Sand" },
};

const RADII: Record<Radius, { name: string; icon: IconName }> = {
	sharp: { name: "Sharp", icon: "corner-none" },
	soft: { name: "Soft", icon: "corner-small" },
	round: { name: "Round", icon: "corner-large" },
};

const DENSITIES: Record<Density, { name: string; icon: IconName }> = {
	compact: { name: "Compact", icon: "6x6-grid" },
	balanced: { name: "Balanced", icon: "6x4-grid" },
	spacious: { name: "Spacious", icon: "4x4-grid" },
};

const THEMES: Record<Theme, { name: string; icon: IconName }> = {
	system: { name: "System", icon: "theme-system" },
	light: { name: "Light", icon: "theme-light" },
	dark: { name: "Dark", icon: "theme-dark" },
};

/** The guides of a group's pages, one level under its label: the last one ends the label's. */
const pageGuides = (count: number) =>
	treeGuides([{ depth: 0 }, ...Array.from({ length: count }, () => ({ depth: 1 }))]).slice(1);

/** The site's look, in a card of its own under the pages, as but.dev's settings are: a row for
 * each setting with its current value at the end, opening a menu of the choices beside it. */
const Appearance: FC = () => {
	const [folded, setFolded] = useState(false);
	const [radius, setRadius] = useState(savedRadius);
	const [density, setDensity] = useState(savedDensity);
	const [theme, setTheme] = useState(savedTheme);
	const [accent, setAccent] = useState(savedAccent);
	const [ground, setGround] = useState(savedGround);
	// Not kept on its own: the corners and density make it, or make it Custom.
	const preset = presetOf(radius, density);

	return (
		<section
			aria-label="Appearance"
			className="docs-site-card"
			data-folded={folded ? "" : undefined}
		>
			<div className="docs-site-appearance-head">
				<NavItem
					leadingRow
					name="Appearance"
					icon="contrast"
					folded={folded}
					onToggleFolded={() => setFolded(!folded)}
				/>
			</div>
			{!folded && (
				<div className="docs-site-appearance-rows">
					<Setting name="Preset" icon="mixer" value={preset?.name ?? "Custom"}>
						<MenuRadioGroup
							value={preset?.id ?? ""}
							onValueChange={(next) => {
								const picked = PRESETS.find(({ id }) => id === next);
								if (picked === undefined) return;
								setRadius(picked.radius);
								setDensity(picked.density);
								pickPreset(picked);
							}}
						>
							{PRESETS.map(({ id, name }) => (
								<MenuRadioItem key={id} value={id}>
									{name}
								</MenuRadioItem>
							))}
						</MenuRadioGroup>
					</Setting>
					<Setting name="Radius" icon={RADII[radius].icon} value={RADII[radius].name}>
						<MenuRadioGroup
							value={radius}
							onValueChange={(next) => {
								setRadius(next as Radius);
								pickRadius(next as Radius);
							}}
						>
							{Object.entries(RADII).map(([value, { name, icon }]) => (
								<MenuRadioItem key={value} value={value} icon={icon}>
									{name}
								</MenuRadioItem>
							))}
						</MenuRadioGroup>
					</Setting>
					<Setting name="Density" icon={DENSITIES[density].icon} value={DENSITIES[density].name}>
						<MenuRadioGroup
							value={density}
							onValueChange={(next) => {
								setDensity(next as Density);
								pickDensity(next as Density);
							}}
						>
							{Object.entries(DENSITIES).map(([value, { name, icon }]) => (
								<MenuRadioItem key={value} value={value} icon={icon}>
									{name}
								</MenuRadioItem>
							))}
						</MenuRadioGroup>
					</Setting>
					<Setting name="Theme" icon={THEMES[theme].icon} value={THEMES[theme].name}>
						<MenuRadioGroup
							value={theme}
							onValueChange={(next) => {
								setTheme(next as Theme);
								pickTheme(next as Theme);
							}}
						>
							{Object.entries(THEMES).map(([value, { name, icon }]) => (
								<MenuRadioItem key={value} value={value} icon={icon}>
									{name}
								</MenuRadioItem>
							))}
						</MenuRadioGroup>
					</Setting>
					<Setting
						name="Accent"
						icon={<span className="docs-site-swatch" />}
						value={ACCENTS.find((a) => a.accent === accent)?.name ?? accent}
					>
						<MenuRadioGroup
							value={accent}
							onValueChange={(next) => {
								setAccent(next as Accent);
								pickAccent(next as Accent);
							}}
						>
							{ACCENTS.map(({ name, accent: value, hex }) => (
								<MenuRadioItem
									key={name}
									value={value}
									leading={<span className="docs-site-swatch" style={{ color: hex }} />}
								>
									{name}
								</MenuRadioItem>
							))}
						</MenuRadioGroup>
					</Setting>
					<Setting
						name="Ground"
						icon={<span className="docs-site-swatch" style={{ color: `var(--but-${ground}-9)` }} />}
						value={GROUNDS[ground].name}
					>
						<MenuRadioGroup
							value={ground}
							onValueChange={(next) => {
								setGround(next as Ground);
								pickGround(next as Ground);
							}}
						>
							{Object.entries(GROUNDS).map(([value, { name }]) => (
								<MenuRadioItem
									key={value}
									value={value}
									leading={
										<span className="docs-site-swatch" style={{ color: `var(--but-${value}-9)` }} />
									}
								>
									{name}
								</MenuRadioItem>
							))}
						</MenuRadioGroup>
					</Setting>
				</div>
			)}
		</section>
	);
};

/** One setting: its name, its glyph and its current value, and the menu of choices it opens. */
const Setting: FC<{
	name: string;
	icon: IconName | ReactElement;
	value: string;
	children: ReactNode;
}> = ({ name, icon, value, children }) => (
	<Menu
		side="right"
		align="start"
		trigger={
			<NavItem
				name={name}
				icon={icon}
				meta={value}
				render={
					// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- The row renders as this button, and NavItem fills it with the name.
					<button type="button" />
				}
			/>
		}
	>
		{children}
	</Menu>
);
