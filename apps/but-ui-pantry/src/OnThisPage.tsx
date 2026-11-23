import { GroupLabel, NavItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
import { treeGuides } from "@gitbutler/but-ui-react/treeGuides.ts";
import { useCallback, useState, type FC, type MouseEvent } from "react";
import { reveal } from "./site.ts";

/** A block the menu lists: a heading, or a rule that opens with a bold lead. */
type Entry = { node: HTMLElement; name: string; rule: boolean };

/** A heading and the rules under it; rules before any heading have none. */
type Section = { head: Entry | undefined; rules: Array<Entry> };

/**
 * What the menu lists, read off the page as drawn so its anchors are the copy links' own: every
 * linked heading and rule, and a design note's own headings, which have no link but head its rules.
 */
const LISTED = ".docs-linked, .docs-site-prose > .text-prose > :is(h2, h3)";

/** A block's title, as the menu names it: a heading's words, or a rule's lead without its stop. */
const nameOf = (node: HTMLElement) => {
	const title = node.matches("p")
		? node.querySelector(":scope > strong")
		: (node.querySelector(":scope > .docs-linked-title") ?? node);
	return (title?.textContent ?? "").trim().replace(/[.,:;]+$/, "");
};

const listed = (root: HTMLElement): Array<Entry> =>
	[...root.querySelectorAll<HTMLElement>(LISTED)].map((node) => ({
		node,
		name: nameOf(node),
		rule: node.matches("p"),
	}));

const same = (a: Array<Entry>, b: Array<Entry>) =>
	a.length === b.length && a.every((entry, index) => entry.node === b[index]?.node);

/** The entries under their headings. */
const sectionsOf = (entries: Array<Entry>) =>
	entries.reduce<Array<Section>>((sections, entry) => {
		if (!entry.rule) sections.push({ head: entry, rules: [] });
		else if (sections.length === 0) sections.push({ head: undefined, rules: [entry] });
		else sections.at(-1)?.rules.push(entry);
		return sections;
	}, []);

/** The rows of the menu: every rule, and a linked heading with none under it. */
const rowsOf = (entries: Array<Entry>) =>
	sectionsOf(entries).flatMap(({ head, rules }) =>
		rules.length > 0 ? rules : head !== undefined && head.node.id !== "" ? [head] : [],
	);

/**
 * The row of the block scrolled to: the last whose top has passed the scroller's, give or take a
 * line, and the first before any has.
 */
const currentOf = (root: HTMLElement, entries: Array<Entry>) => {
	const line = root.getBoundingClientRect().top + 96;
	const rows = rowsOf(entries);
	let current = rows[0];
	for (const row of rows) {
		if (row.node.getBoundingClientRect().top > line) break;
		current = row;
	}
	return current?.node;
};

const guidesFor = (count: number) =>
	treeGuides([{ depth: 0 }, ...Array.from({ length: count }, () => ({ depth: 1 }))]).slice(1);

/**
 * The page's own menu, beside it when the window has room: its headings, and the rules under
 * them, as the sidebar lists the pages, in a card that stays put while the page scrolls. The row
 * of the block scrolled to is selected; pressing a row goes there and lights its title, as
 * following a link to it does. It reads the page as drawn and listens for it to change, as when a
 * tab is picked, so any page gets one without saying what's on it.
 */
export const OnThisPage: FC = () => {
	const [entries, setEntries] = useState<Array<Entry>>([]);
	const [current, setCurrent] = useState<HTMLElement | undefined>();
	const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());

	const watch = useCallback((rail: HTMLElement | null) => {
		const root = rail?.closest<HTMLElement>(".docs-site-main");
		if (root === null || root === undefined) return;
		let list: Array<Entry> = [];
		const spy = () => setCurrent(currentOf(root, list));
		const relist = () => {
			const next = listed(root);
			if (!same(list, next)) setEntries((list = next));
			spy();
		};
		const observer = new MutationObserver(relist);
		observer.observe(root, { childList: true, subtree: true });
		root.addEventListener("scroll", spy, { passive: true });
		relist();
		return () => {
			observer.disconnect();
			root.removeEventListener("scroll", spy);
		};
	}, []);

	const toggle = (key: string) =>
		setFolded((previous) => {
			const next = new Set(previous);
			if (!next.delete(key)) next.add(key);
			return next;
		});

	const go = (node: HTMLElement) => (event: MouseEvent<HTMLAnchorElement>) => {
		if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
			return;
		event.preventDefault();
		const address = new URL(window.location.href);
		address.hash = node.id;
		window.history.replaceState(null, "", address);
		reveal(node);
	};

	const row = (entry: Entry, depth: number, guides?: ReturnType<typeof guidesFor>[number]) => (
		<NavItem
			key={entry.node.id}
			name={entry.name}
			tooltip={entry.name}
			depth={depth}
			guides={guides}
			selected={entry.node === current}
			render={
				// oxlint-disable-next-line jsx_a11y/anchor-has-content, jsx_a11y/control-has-associated-label -- The row renders as this link, and NavItem fills it with the name.
				<a
					href={`#${entry.node.id}`}
					aria-current={entry.node === current ? "location" : undefined}
					onClick={go(entry.node)}
				/>
			}
		/>
	);

	return (
		<aside ref={watch} className="docs-site-rail" aria-label="On this page">
			{rowsOf(entries).length > 1 && (
				// The block scrolled to is the selection, so it takes the solid fill of a focused one.
				<nav className="docs-site-card docs-site-rail-card" data-selection-focused="true">
					{sectionsOf(entries).map(({ head, rules }, index) => {
						const key = `${index}:${head?.name ?? ""}`;
						if (rules.length === 0)
							return head !== undefined && head.node.id !== "" ? row(head, 0) : null;
						return (
							<div key={key} className="docs-site-rail-section">
								{head !== undefined && (
									<GroupLabel
										name={head.name}
										folded={folded.has(key)}
										onToggleFolded={() => toggle(key)}
									/>
								)}
								{!folded.has(key) &&
									rules.map((rule, at) =>
										head === undefined ? row(rule, 0) : row(rule, 1, guidesFor(rules.length)[at]),
									)}
							</div>
						);
					})}
				</nav>
			)}
		</aside>
	);
};
