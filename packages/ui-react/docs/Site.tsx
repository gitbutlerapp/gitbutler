import { FieldControlWithIcon } from "../src/Field.tsx";
import { Icon } from "../src/Icon.tsx";
import { ScrollArea } from "../src/ScrollArea.tsx";
import { Segment, SegmentedControl } from "../src/SegmentedControl.tsx";
import { GroupLabel, NavItem } from "../src/SidebarRow.tsx";
import { ComponentPage } from "./ComponentPage.tsx";
import { DesignPage } from "./DesignPage.tsx";
import { groupPages, loadPage, pageName, pages, type Page } from "./pages.ts";
import { currentPage, pageLink, pickTheme, savedTheme, subscribePage, type Theme } from "./site.ts";
import { Suspense, use, useState, useSyncExternalStore, type FC } from "react";

/** The docs site: the pages under their headings, a search over them, the theme, and the page. */
export const Site: FC = () => {
	const page = useSyncExternalStore(subscribePage, currentPage);

	return (
		<div className="docs-site">
			<aside className="docs-site-sidebar">
				<Navigation current={page} />
				<ThemeSwitch />
			</aside>
			<main className="docs-site-main">
				<Suspense>{page !== undefined && <PageView key={page.id} page={page} />}</Suspense>
			</main>
		</div>
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
		<>
			<div className="docs-site-top">
				<div className="text-14 text-semibold">GitButler UI</div>
				<FieldControlWithIcon
					icon={<Icon name="search" size={16} />}
					type="search"
					placeholder="Search…"
					aria-label="Search the pages"
					className="docs-site-search"
					value={query}
					onChange={(event) => setQuery(event.target.value)}
				/>
			</div>
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
		</>
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
