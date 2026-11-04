import { PopupSearch } from "../src/Popup.tsx";
import { ResizeHandle } from "../src/ResizeHandle.tsx";
import { ScrollArea } from "../src/ScrollArea.tsx";
import { Segment, SegmentedControl } from "../src/SegmentedControl.tsx";
import { GroupLabel, NavItem } from "../src/SidebarRow.tsx";
import { ComponentPage } from "./ComponentPage.tsx";
import { DesignPage } from "./DesignPage.tsx";
import { groupPages, loadPage, pageName, pages, type Page } from "./pages.ts";
import { currentPage, pageLink, pickTheme, savedTheme, subscribePage, type Theme } from "./site.ts";
import { Suspense, use, useState, useSyncExternalStore, type FC } from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";

/**
 * The docs site: the pages under their headings, a search over them, the theme, and the page. The
 * sidebar is as wide as it was last dragged.
 */
export const Site: FC = () => {
	const page = useSyncExternalStore(subscribePage, currentPage);
	const layout = useDefaultLayout({ id: "docs-site", storage: localStorage });

	return (
		<Group orientation="horizontal" className="docs-site" style={{ height: "100vh" }} {...layout}>
			<Panel id="sidebar" defaultSize="280px" minSize="220px" maxSize="400px">
				<aside className="docs-site-sidebar">
					<ScrollArea className="docs-site-scroll" viewportClassName="docs-site-scroll-viewport">
						<Navigation current={page} />
					</ScrollArea>
					<div className="docs-site-footer">
						<div className="docs-site-card docs-site-theme">
							<ThemeSwitch />
						</div>
					</div>
				</aside>
			</Panel>
			<ResizeHandle />
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

const Navigation: FC<{ current: Page | undefined }> = ({ current }) => {
	const [query, setQuery] = useState("");
	const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());
	const found = pages.filter((page) =>
		page.title.toLowerCase().includes(query.trim().toLowerCase()),
	);
	const toggle = (heading: string) =>
		setFolded((previous) => {
			const next = new Set(previous);
			if (!next.delete(heading)) next.add(heading);
			return next;
		});

	return (
		<div>
			<div className="docs-site-pages-head">
				<div className="docs-site-pages-top">
					<PopupSearch
						placeholder="Search…"
						aria-label="Search the pages"
						value={query}
						onChange={(event) => setQuery(event.target.value)}
						onClear={query === "" ? undefined : () => setQuery("")}
					/>
				</div>
			</div>
			{/* The page shown is the selection, so it takes the solid fill of a focused one. */}
			<nav className="docs-site-pages" aria-label="Pages" data-selection-focused="true">
				{groupPages(found).map(([heading, group]) => (
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
			</nav>
			<div className="docs-site-pages-foot">
				<div className="docs-site-pages-bottom" />
			</div>
		</div>
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
