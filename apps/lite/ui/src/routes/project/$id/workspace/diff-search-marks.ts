import type { CodeViewOptions, SelectionSide } from "@pierre/diffs";
import { useLayoutEffect, useRef } from "react";
import { diffLineTargetFromElement } from "./diff-line-target.ts";
import type { DiffSearchMatch } from "./diff-search.ts";

type OnPostRender<T> = NonNullable<CodeViewOptions<T>["onPostRender"]>;

const SEARCH_MATCH_ATTRIBUTE = "data-gitbutler-search-match";

/**
 * A light wash on every matching line, and a stronger one across the whole
 * current match — contrast alone tells the two apart.
 */
export const diffSearchMarksUnsafeCSS = `
	[data-line][${SEARCH_MATCH_ATTRIBUTE}] {
		background-color: color-mix(var(--fill-warn-bg) 14%, transparent);
	}

	[data-column-number][${SEARCH_MATCH_ATTRIBUTE}] {
		background-color: color-mix(var(--fill-warn-bg) 32%, transparent);
	}

	[data-line][${SEARCH_MATCH_ATTRIBUTE}="current"] {
		background-color: color-mix(var(--fill-warn-bg) 34%, transparent);
	}

	[data-column-number][${SEARCH_MATCH_ATTRIBUTE}="current"] {
		background-color: color-mix(var(--fill-warn-bg) 70%, transparent);
	}
`;

const markKey = (itemId: string, side: SelectionSide, lineNumber: number): string =>
	`${itemId} ${side} ${lineNumber}`;

const keysOf = (match: DiffSearchMatch): Array<string> => {
	const keys = [markKey(match.itemId, match.side, match.lineNumber)];
	if (match.deletionsColumnLine !== undefined)
		keys.push(markKey(match.itemId, "deletions", match.deletionsColumnLine));
	return keys;
};

/** What matched, and which of those the search is standing on. */
export type SearchMarks = {
	matches: Array<DiffSearchMatch>;
	current: DiffSearchMatch | null;
};

const NO_MARKS: SearchMarks = { matches: [], current: null };

type SearchMarkStore<T> = {
	onPostRender: OnPostRender<T>;
	setMatches: (matches: Array<DiffSearchMatch>, current: DiffSearchMatch | null) => void;
	/** For React consumers painting their own view of the matches — the minimap. */
	subscribe: (listener: () => void) => () => void;
	getSnapshot: () => SearchMarks;
	cleanUp: () => void;
};

/**
 * Stamps every rendered line that holds a search match, the way the gutter
 * store stamps its controls: the virtualizer re-renders items as the window
 * moves, and each post-render re-walks that item's lines against the current
 * match set. A query change re-walks every registered item instead, since no
 * render happens on its behalf.
 */
const createSearchMarkStore = <T>(getOnPostRender: () => OnPostRender<T>): SearchMarkStore<T> => {
	const itemIdsByHost = new Map<HTMLElement, string>();
	const listeners = new Set<() => void>();
	let keys: ReadonlySet<string> = new Set();
	let currentKeys: ReadonlySet<string> = new Set();
	let snapshot: SearchMarks = NO_MARKS;

	const markHost = (host: HTMLElement, itemId: string): void => {
		const shadowRoot = host.shadowRoot;
		if (!shadowRoot) return;

		for (const element of shadowRoot.querySelectorAll<HTMLElement>(
			"[data-column-number], [data-line]",
		)) {
			const target = diffLineTargetFromElement({ element, itemId });
			const key = target === null ? null : markKey(target.itemId, target.side, target.lineNumber);
			if (key !== null && currentKeys.has(key))
				element.setAttribute(SEARCH_MATCH_ATTRIBUTE, "current");
			else if (key !== null && keys.has(key)) element.setAttribute(SEARCH_MATCH_ATTRIBUTE, "");
			else element.removeAttribute(SEARCH_MATCH_ATTRIBUTE);
		}
	};

	return {
		onPostRender: (host, instance, phase, context) => {
			if (phase === "unmount" || context.type !== "diff") {
				itemIdsByHost.delete(host);
			} else {
				itemIdsByHost.set(host, context.item.id);
				markHost(host, context.item.id);
			}
			// CodeView exposes this callback as file/diff overloads; forward the exact invocation.
			Reflect.apply(getOnPostRender(), undefined, [host, instance, phase, context]);
		},
		setMatches: (matches, current) => {
			keys = new Set(matches.flatMap(keysOf));
			currentKeys = new Set(current === null ? [] : keysOf(current));
			for (const [host, itemId] of itemIdsByHost) markHost(host, itemId);

			// A fresh identity only when there is something to draw, so a closed
			// search doesn't wake subscribers on every diff refresh.
			snapshot = matches.length === 0 ? NO_MARKS : { matches, current };
			for (const listener of listeners) listener();
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		getSnapshot: () => snapshot,
		cleanUp: () => {
			keys = new Set();
			currentKeys = new Set();
			for (const [host, itemId] of itemIdsByHost) markHost(host, itemId);
			itemIdsByHost.clear();
			snapshot = NO_MARKS;
			listeners.clear();
		},
	};
};

export const useDiffSearchMarks = <T>(
	onPostRender: OnPostRender<T>,
): {
	onPostRender: OnPostRender<T>;
	setSearchMatches: SearchMarkStore<T>["setMatches"];
	/** Hand to the minimap, which draws the same matches at its own scale. */
	searchMarks: Pick<SearchMarkStore<T>, "subscribe" | "getSnapshot">;
} => {
	const onPostRenderRef = useRef(onPostRender);
	onPostRenderRef.current = onPostRender;

	const storeRef = useRef<SearchMarkStore<T>>(null);
	storeRef.current ??= createSearchMarkStore(() => onPostRenderRef.current);
	const store = storeRef.current;

	useLayoutEffect(() => () => store.cleanUp(), [store]);

	return {
		onPostRender: store.onPostRender,
		setSearchMatches: store.setMatches,
		searchMarks: store,
	};
};
