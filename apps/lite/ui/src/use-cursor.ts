import {
	cursorKey,
	type ListItem,
	type ListName,
	type WorkspaceCursorSnapshot,
} from "#ui/cursors.ts";
import {
	encodeCursorParam,
	isUrlList,
	type UrlListName,
	type UrlQueryParams,
} from "#ui/cursor-url.ts";
import { isValidOutlineModeForSelection } from "#ui/outline/mode.ts";
import { branchOperand, commitOperand } from "#ui/operands.ts";
import type { BranchOperand, CommitOperand, Operand } from "#ui/operands.ts";
import type { OutlineTab, WorkspaceList } from "#ui/projects/project.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { writeLastPlace } from "#ui/project.ts";
import { router } from "#ui/router.ts";
import { store, useAppSelector } from "#ui/store.ts";
import {
	resolveNavigationIndexSelection,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";
import { useSearch } from "@tanstack/react-router";
import { useEffect } from "react";

/**
 * The one way in and out of navigation state. The URL holds the page, the
 * driving list and every addressable cursor; the store holds only the diff
 * cursor (see cursor-url.ts for why). Callers never see the split: reads are
 * hooks here, writes are plain calls — the router and the store are both
 * module-level, so moving a cursor needs no dispatch and no hook.
 */

const WORKSPACE_ROUTE = "/project/$id/workspace" as const;

// Indexing the per-list map with a union collapses it to an intersection;
// this view is what the generic paths below call.
const encodeUnion = encodeCursorParam as (
	list: UrlListName,
	item: ListItem[UrlListName],
) => string | null;

/**
 * The project the app is showing. Read here rather than passed in by callers,
 * which only holds because one project is on screen at a time.
 */
const projectIdOf = (): string => {
	const match = router.state.matches.findLast((m) => "id" in m.params);
	if (!match) throw new Error("No project route matched");
	return (match.params as { id: string }).id;
};

/** The params as they stand, for callbacks — components subscribe via the hooks below. */
export const currentParams = (): UrlQueryParams => router.state.location.search as UrlQueryParams;

/* ------------------------------------------------------------------ reads */

/**
 * Resolution is an encode-match: the param is compared against each index
 * item's encoded form, so `change:<id>` still finds its commit after a
 * rewrite gave it a new id — the whole reason commits are change-id-first.
 * Cached per index identity; indexes are rebuilt when their data changes.
 */
const encodedIndexes = new WeakMap<object, Map<string, unknown>>();

const encodedIndex = <L extends UrlListName>(
	list: L,
	navigationIndex: NavigationIndex<ListItem[L]>,
): Map<string, ListItem[L]> => {
	let byParam = encodedIndexes.get(navigationIndex);
	if (!byParam) {
		byParam = new Map();
		for (const item of navigationIndex.items) {
			const encoded = encodeCursorParam(list, item);
			if (encoded !== null && !byParam.has(encoded)) byParam.set(encoded, item);
		}
		encodedIndexes.set(navigationIndex, byParam);
	}
	return byParam as Map<string, ListItem[L]>;
};

const resolveCursorParam = <L extends UrlListName>(
	list: L,
	param: string | undefined,
	navigationIndex: NavigationIndex<ListItem[L]>,
): ListItem[L] | null =>
	(param === undefined ? undefined : encodedIndex(list, navigationIndex).get(param)) ??
	navigationIndex.items[0] ??
	null;

/** The cursor resolved against what the list currently shows. */
export const useResolvedCursor = <L extends ListName>(
	list: L,
	navigationIndex: NavigationIndex<ListItem[L]>,
): ListItem[L] | null => {
	// Both stores are subscribed unconditionally so hook order never depends
	// on the list; every call site passes a literal list name anyway.
	const param = useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => (isUrlList(list) ? params[list as UrlListName] : undefined),
	});
	const storedDiff = useAppSelector((state) =>
		projectSlice.selectors.selectDiffCursor(state, projectIdOf()),
	);

	return (
		isUrlList(list)
			? resolveCursorParam(list, param, navigationIndex as never)
			: resolveNavigationIndexSelection(
					navigationIndex as NavigationIndex<ListItem["diff"]>,
					storedDiff,
					cursorKey.diff,
				)
	) as ListItem[L] | null;
};

/**
 * Whether the resolved cursor rests on `item`. A primitive, so a cursor move
 * re-renders the two affected rows rather than the whole list.
 */
export const useIsCursorAt = <L extends ListName>(
	list: L,
	navigationIndex: NavigationIndex<ListItem[L]>,
	item: ListItem[L],
): boolean => {
	const resolved = useResolvedCursor(list, navigationIndex);
	return resolved !== null && cursorKey[list](resolved) === cursorKey[list](item);
};

/**
 * Whether the stored cursor is `item`, without resolving against an index.
 * Rows subscribe to this plain boolean so index rebuilds (fold, filter, data
 * refresh) do not re-render every row.
 */
export const useCursorMatches = <L extends UrlListName>(list: L, item: ListItem[L]): boolean =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => params[list] === encodeCursorParam(list, item),
	});

/** The outline page shown, `workspace` unless the URL says otherwise. */
export const usePage = (): OutlineTab =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => params.page ?? "workspace",
	});

const pageOf = (): OutlineTab => currentParams().page ?? "workspace";

/** The workspace page's driving list, `stacks` unless the URL says otherwise. */
export const useWorkspaceList = (): WorkspaceList =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => params.list ?? "stacks",
	});

const workspaceListOf = (): WorkspaceList => currentParams().list ?? "stacks";

const drivenByUncommitted = (params: UrlQueryParams): boolean =>
	(params.page ?? "workspace") === "workspace" && (params.list ?? "stacks") === "uncommitted";

/**
 * The uncommitted list is itself a file list, so while it drives the pane a
 * second files panel would just repeat it.
 */
export const useCanShowFiles = (): boolean =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => !drivenByUncommitted(params),
	});

export const outlineSelectionScopeOf = (): "outline" | "uncommitted-files" =>
	drivenByUncommitted(currentParams()) ? "uncommitted-files" : "outline";

/** The focus scope of the outline panel's driving list. */
export const useOutlineSelectionScope = (): "outline" | "uncommitted-files" =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) =>
			drivenByUncommitted(params) ? ("uncommitted-files" as const) : ("outline" as const),
	});

/* ----------------------------------------------------------------- writes */

/** Within-page state never creates history entries (ruled 2026-08-13). */
const navigateParams = (update: (prev: UrlQueryParams) => UrlQueryParams): void => {
	// Applied against the live params rather than handed to the router as an
	// updater: a write triggered by data arriving (a list resolving its cursor)
	// would otherwise be given a `prev` snapshotted before the URL was parsed,
	// dropping whatever it had not yet seen — the page param, most visibly.
	void router.navigate({ to: ".", search: update(currentParams()), replace: true }).then(() => {
		writeLastPlace(projectIdOf(), router.state.location.searchStr);
	});
};

const setDiffCursor = (selection: ListItem["diff"] | null): void => {
	store.dispatch(projectSlice.actions.selectDiffCursor({ projectId: projectIdOf(), selection }));
};

/** A stacks selection dissolves an outline mode it invalidates, as before. */
const dissolveInvalidMode = (selection: Operand | null): void => {
	const mode = projectSlice.selectors.selectOutlineModeState(store.getState(), projectIdOf());
	if (mode._tag === "Default") return;

	if (!selection || !isValidOutlineModeForSelection({ mode, selection }))
		store.dispatch(projectSlice.actions.exitMode({ projectId: projectIdOf() }));
};

/** Move a list's cursor. */
export const setCursor = <L extends ListName>(list: L, item: ListItem[L] | null): void => {
	if (!isUrlList(list)) {
		setDiffCursor(item as ListItem["diff"] | null);
		return;
	}

	const encoded =
		item === null ? undefined : (encodeUnion(list, item as ListItem[UrlListName]) ?? undefined);
	// Selecting the same item is a no-op, side effects included; selecting
	// null always lands (it may still have sub-cursors to clear).
	if (item !== null && currentParams()[list] === encoded) return;

	navigateParams((prev) => ({
		...prev,
		[list]: encoded,
		// The file and diff cursors follow whatever the stacks cursor rests on.
		...(list === "stacks" ? { files: undefined } : {}),
	}));

	if (list === "stacks") {
		setDiffCursor(null);
		dissolveInvalidMode(item as Operand | null);
	}
};

/** Switch the outline page. Changing pages dissolves any outline mode. */
export const setPage = (page: OutlineTab): void => {
	if (pageOf() === page) return;

	navigateParams((prev) => ({ ...prev, page: page === "workspace" ? undefined : page }));
	store.dispatch(projectSlice.actions.exitMode({ projectId: projectIdOf() }));
};

/** Name the workspace list that drives the details pane. */
export const setWorkspaceList = (list: WorkspaceList): void => {
	if (workspaceListOf() === list) return;

	navigateParams((prev) => ({ ...prev, list: list === "stacks" ? undefined : list }));
};

/* ------------------------------------------------- modes with restoration */

const snapshotWorkspaceCursors = (): WorkspaceCursorSnapshot => {
	const params = currentParams();
	return {
		stacks: params.stacks,
		uncommitted: params.uncommitted,
		files: params.files,
		diff: projectSlice.selectors.selectDiffCursor(store.getState(), projectIdOf()),
	};
};

const restoreWorkspaceCursors = (snapshot: WorkspaceCursorSnapshot): void => {
	navigateParams((prev) => ({
		...prev,
		stacks: snapshot.stacks,
		uncommitted: snapshot.uncommitted,
		files: snapshot.files,
	}));
	setDiffCursor(snapshot.diff);
};

export const enterKeyboardTransfer = ({
	sources,
	placement,
}: {
	sources: Array<Operand>;
	placement?: "above" | "below" | "into";
}): void => {
	store.dispatch(
		projectSlice.actions.enterKeyboardTransferMode({
			projectId: projectIdOf(),
			sources,
			placement,
			restoreSelection: snapshotWorkspaceCursors(),
		}),
	);
};

export const enterAbsorb = ({
	source,
	sourceTarget,
}: {
	source: Operand;
	sourceTarget: AbsorptionTarget;
}): void => {
	store.dispatch(
		projectSlice.actions.enterAbsorbMode({
			projectId: projectIdOf(),
			source,
			sourceTarget,
			restoreSelection: snapshotWorkspaceCursors(),
		}),
	);
};

/** Leave the mode and put every cursor back where the mode found it. */
export const cancelMode = (): void => {
	const mode = projectSlice.selectors.selectOutlineModeState(store.getState(), projectIdOf());
	const restore =
		mode._tag === "Absorb"
			? mode.restoreSelection
			: mode._tag === "Transfer" && mode.value._tag === "Keyboard"
				? mode.value.restoreSelection
				: null;

	store.dispatch(projectSlice.actions.exitMode({ projectId: projectIdOf() }));
	if (restore) restoreWorkspaceCursors(restore);
};

export const startRewordCommit = (commit: CommitOperand): void => {
	setCursor("stacks", commitOperand(commit));
	store.dispatch(projectSlice.actions.startRewordCommit({ projectId: projectIdOf(), commit }));
};

export const startRenameBranch = (branch: BranchOperand): void => {
	setCursor("stacks", branchOperand(branch));
	store.dispatch(projectSlice.actions.startRenameBranch({ projectId: projectIdOf(), branch }));
};

/* ------------------------------------------------------- rewrite handling */

const operandParams = ["stacks", "branches", "upstream"] as const;

/**
 * Rewrite `commit:` params after a commit rewrite. `change:` params need no
 * repair — the change id survives — which is why they are the primary form.
 */
export const remapSearchCommits = (replacedCommits: Record<string, string>): void => {
	navigateParams((prev) => {
		const next = { ...prev };
		for (const param of operandParams) {
			const value = next[param];
			if (value === undefined || !value.startsWith("commit:")) continue;

			const newId = replacedCommits[value.slice("commit:".length)];
			if (newId !== undefined) next[param] = `commit:${newId}`;
		}
		return next;
	});
};

/** Follow a branch rename in every param that named it. */
export const remapSearchBranch = (oldRef: string, newRef: string): void => {
	navigateParams((prev) => {
		const next = { ...prev };
		for (const param of operandParams)
			if (next[param] === `branch:${oldRef}`) next[param] = `branch:${newRef}`;
		return next;
	});
};

/* ------------------------------------------------------------- write-back */

/**
 * Rows highlight by comparing against the stored cursor, so whenever
 * resolution lands elsewhere — entering the tab, or the cursor's item leaving
 * the index — the list stores the resolved value to keep the two in
 * agreement. One effect for every list.
 */
export const useCursorWriteBack = <L extends ListName>(
	list: L,
	navigationIndex: NavigationIndex<ListItem[L]>,
): void => {
	const resolved = useResolvedCursor(list, navigationIndex);
	const storedParam = useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => (isUrlList(list) ? params[list as UrlListName] : undefined),
	});
	const storedDiff = useAppSelector((state) =>
		projectSlice.selectors.selectDiffCursor(state, projectIdOf()),
	);

	const outOfSync =
		resolved !== null &&
		(isUrlList(list)
			? storedParam !== encodeUnion(list, resolved as ListItem[UrlListName])
			: storedDiff === null ||
				cursorKey.diff(storedDiff) !== cursorKey.diff(resolved as ListItem["diff"]))
			? resolved
			: null;

	useEffect(() => {
		if (outOfSync !== null) setCursor(list, outOfSync);
	}, [outOfSync, list]);
};
