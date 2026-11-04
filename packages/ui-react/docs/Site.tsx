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
			<Panel id="sidebar" defaultSize="240px" minSize="200px" maxSize="400px">
				<aside className="docs-site-sidebar">
					<Navigation current={page} />
					<ThemeSwitch />
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
		<div className="docs-site-card">
			<PopupSearch
				placeholder="Search…"
				aria-label="Search the pages"
				value={query}
				onChange={(event) => setQuery(event.target.value)}
				onClear={query === "" ? undefined : () => setQuery("")}
			/>
			<ScrollArea className="docs-site-scroll">
				<nav className="docs-site-nav" aria-label="Pages">
					{groupPages(found).map(([heading, group]) => (
						<div key={heading}>
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
			</ScrollArea>
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
		<SegmentedControl
			value={theme}
			onValueChange={pick}
			aria-label="Theme"
			className="docs-site-theme"
		>
			<Segment value="light">Light</Segment>
			<Segment value="dark">Dark</Segment>
		</SegmentedControl>
	);
};
