import {
	cursorKey,
	type CursorItem,
	type CursorName,
	type WorkspaceCursorSnapshot,
} from "#ui/cursors.ts";
import {
	encodeCursorParam,
	isUrlCursor,
	type UrlCursorName,
	type UrlQueryParams,
} from "#ui/cursor-url.ts";
import {
	isValidPendingOperationForSelection,
	type InlineEditOperand,
} from "#ui/operations/pending-operation.ts";
import type { Operand } from "#ui/operands.ts";
import type { PageId, ActiveList } from "#ui/projects/project.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { writeLastPlace } from "#ui/project.ts";
import { router } from "#ui/router.ts";
import { store, useAppSelector } from "#ui/store.ts";
import {
	resolveNavigationIndexSelection,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";
import type { TransferKind } from "#ui/operations/operation.ts";
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
	list: UrlCursorName,
	item: CursorItem[UrlCursorName],
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

const encodedIndex = <L extends UrlCursorName>(
	list: L,
	navigationIndex: NavigationIndex<CursorItem[L]>,
): Map<string, CursorItem[L]> => {
	let byParam = encodedIndexes.get(navigationIndex);
	if (!byParam) {
		byParam = new Map();
		for (const item of navigationIndex.items) {
			const encoded = encodeCursorParam(list, item);
			if (encoded !== null && !byParam.has(encoded)) byParam.set(encoded, item);
		}
		encodedIndexes.set(navigationIndex, byParam);
	}
	return byParam as Map<string, CursorItem[L]>;
};

const resolveCursorParam = <L extends UrlCursorName>(
	list: L,
	param: string | undefined,
	navigationIndex: NavigationIndex<CursorItem[L]>,
): CursorItem[L] | null =>
	(param === undefined ? undefined : encodedIndex(list, navigationIndex).get(param)) ??
	navigationIndex.items[0] ??
	null;

/** The cursor resolved against what the list currently shows. */
export const useSelection = <L extends CursorName>(
	list: L,
	navigationIndex: NavigationIndex<CursorItem[L]>,
): CursorItem[L] | null => {
	// Both stores are subscribed unconditionally so hook order never depends
	// on the list; every call site passes a literal list name anyway.
	const param = useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) =>
			isUrlCursor(list) ? params[list as UrlCursorName] : undefined,
	});
	const storedDiff = useAppSelector((state) =>
		projectSlice.selectors.selectDiffCursor(state, projectIdOf()),
	);

	return (
		isUrlCursor(list)
			? resolveCursorParam(list, param, navigationIndex as never)
			: resolveNavigationIndexSelection(
					navigationIndex as NavigationIndex<CursorItem["diff"]>,
					storedDiff,
					cursorKey.diff,
				)
	) as CursorItem[L] | null;
};

/**
 * Whether the resolved cursor rests on `item`. A primitive, so a cursor move
 * re-renders the two affected rows rather than the whole list.
 */
export const useIsCursorAt = <L extends CursorName>(
	list: L,
	navigationIndex: NavigationIndex<CursorItem[L]>,
	item: CursorItem[L],
): boolean => {
	const resolved = useSelection(list, navigationIndex);
	return resolved !== null && cursorKey[list](resolved) === cursorKey[list](item);
};

/**
 * Whether the stored cursor is `item`, without resolving against an index.
 * Rows subscribe to this plain boolean so index rebuilds (fold, filter, data
 * refresh) do not re-render every row.
 */
export const useCursorMatches = <L extends UrlCursorName>(list: L, item: CursorItem[L]): boolean =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => params[list] === encodeCursorParam(list, item),
	});

/** The sidebar page shown, `workspace` unless the URL says otherwise. */
export const usePage = (): PageId =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => params.page ?? "workspace",
	});

const pageOf = (): PageId => currentParams().page ?? "workspace";

/** The workspace page's driving list, `stacks` unless the URL says otherwise. */
export const useActiveList = (): ActiveList =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => params.active ?? "applied",
	});

const activeListOf = (): ActiveList => currentParams().active ?? "applied";

const drivenByUncommitted = (params: UrlQueryParams): boolean =>
	(params.page ?? "workspace") === "workspace" && (params.active ?? "applied") === "uncommitted";

/**
 * The uncommitted list is itself a file list, so while it drives the pane a
 * second files panel would just repeat it.
 */
export const useCanShowFiles = (): boolean =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) => !drivenByUncommitted(params),
	});

export const sidebarFocusScopeOf = (): "sidebar" | "uncommitted-files" =>
	drivenByUncommitted(currentParams()) ? "uncommitted-files" : "sidebar";

/** The focus scope of the sidebar's driving list. */
export const useSidebarFocusScope = (): "sidebar" | "uncommitted-files" =>
	useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) =>
			drivenByUncommitted(params) ? ("uncommitted-files" as const) : ("sidebar" as const),
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

const setDiffCursor = (selection: CursorItem["diff"] | null): void => {
	store.dispatch(projectSlice.actions.selectDiffCursor({ projectId: projectIdOf(), selection }));
};

/** A stacks selection dissolves a pending operation it invalidates, as before. */
const dissolveInvalidOperation = (selection: Operand | null): void => {
	const pendingOperation = projectSlice.selectors.selectPendingOperation(
		store.getState(),
		projectIdOf(),
	);
	if (pendingOperation._tag === "None") return;

	if (!selection || !isValidPendingOperationForSelection({ pendingOperation, selection }))
		store.dispatch(projectSlice.actions.clearPendingOperation({ projectId: projectIdOf() }));
};

/** Move a list's cursor. */
export const setCursor = <L extends CursorName>(list: L, item: CursorItem[L] | null): void => {
	if (!isUrlCursor(list)) {
		setDiffCursor(item as CursorItem["diff"] | null);
		return;
	}

	const encoded =
		item === null ? undefined : (encodeUnion(list, item as CursorItem[UrlCursorName]) ?? undefined);
	// Selecting the same item is a no-op, side effects included; selecting
	// null always lands (it may still have sub-cursors to clear).
	if (item !== null && currentParams()[list] === encoded) return;

	navigateParams((prev) => ({
		...prev,
		[list]: encoded,
		// The file and diff cursors follow whatever the stacks cursor rests on.
		...(list === "applied" ? { files: undefined } : {}),
	}));

	if (list === "applied") {
		setDiffCursor(null);
		dissolveInvalidOperation(item as Operand | null);
	}
};

/** Switch the sidebar page. Changing pages dissolves any pending operation. */
export const setPage = (page: PageId): void => {
	if (pageOf() === page) return;

	navigateParams((prev) => ({ ...prev, page: page === "workspace" ? undefined : page }));
	store.dispatch(projectSlice.actions.clearPendingOperation({ projectId: projectIdOf() }));
};

/** Name the workspace list that drives the details pane. */
export const setActiveList = (list: ActiveList): void => {
	if (activeListOf() === list) return;

	navigateParams((prev) => ({ ...prev, active: list === "applied" ? undefined : list }));
};

/* -------------------------- pending operations with restoration */

const snapshotWorkspaceCursors = (): WorkspaceCursorSnapshot => {
	const params = currentParams();
	return {
		page: params.page,
		active: params.active,
		applied: params.applied,
		uncommitted: params.uncommitted,
		files: params.files,
		diff: projectSlice.selectors.selectDiffCursor(store.getState(), projectIdOf()),
	};
};

const restoreWorkspaceCursors = (snapshot: WorkspaceCursorSnapshot): void => {
	navigateParams((prev) => ({
		...prev,
		page: snapshot.page,
		active: snapshot.active,
		applied: snapshot.applied,
		uncommitted: snapshot.uncommitted,
		files: snapshot.files,
	}));
	setDiffCursor(snapshot.diff);
};

export const startKeyboardTransfer = ({
	sources,
	kind,
	placement,
}: {
	sources: Array<Operand>;
	kind: TransferKind;
	placement?: "above" | "below" | "into";
}): void => {
	const restoreSelection = snapshotWorkspaceCursors();
	if (restoreSelection.page !== undefined)
		navigateParams((prev) => ({ ...prev, page: undefined, active: undefined }));

	store.dispatch(
		projectSlice.actions.startKeyboardTransfer({
			projectId: projectIdOf(),
			sources,
			kind,
			placement,
			restoreSelection,
		}),
	);
};

export const startAbsorb = ({
	sources,
	sourceTarget,
}: {
	sources: Array<Operand>;
	sourceTarget: AbsorptionTarget;
}): void => {
	store.dispatch(
		projectSlice.actions.startAbsorb({
			projectId: projectIdOf(),
			sources,
			sourceTarget,
			restoreSelection: snapshotWorkspaceCursors(),
		}),
	);
};

/** Cancel the pending operation and put every cursor back where it found them. */
export const cancelPendingOperation = (): void => {
	const pending = projectSlice.selectors.selectPendingOperation(store.getState(), projectIdOf());
	const restore =
		pending._tag === "Absorb"
			? pending.restoreSelection
			: pending._tag === "Transfer" && pending.value._tag === "Keyboard"
				? pending.value.restoreSelection
				: null;

	store.dispatch(projectSlice.actions.clearPendingOperation({ projectId: projectIdOf() }));
	if (restore) restoreWorkspaceCursors(restore);
};

export const startInlineEdit = (operand: InlineEditOperand): void => {
	setCursor("applied", operand);
	store.dispatch(projectSlice.actions.startInlineEdit({ projectId: projectIdOf(), operand }));
};

/* ------------------------------------------------------- rewrite handling */

const operandParams = ["applied", "branches", "upstream"] as const;

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
export const useCursorWriteBack = <L extends CursorName>(
	list: L,
	navigationIndex: NavigationIndex<CursorItem[L]>,
): void => {
	const resolved = useSelection(list, navigationIndex);
	const storedParam = useSearch({
		from: WORKSPACE_ROUTE,
		select: (params: UrlQueryParams) =>
			isUrlCursor(list) ? params[list as UrlCursorName] : undefined,
	});
	const storedDiff = useAppSelector((state) =>
		projectSlice.selectors.selectDiffCursor(state, projectIdOf()),
	);

	const outOfSync =
		resolved !== null &&
		(isUrlCursor(list)
			? storedParam !== encodeUnion(list, resolved as CursorItem[UrlCursorName])
			: storedDiff === null ||
				cursorKey.diff(storedDiff) !== cursorKey.diff(resolved as CursorItem["diff"]))
			? resolved
			: null;

	useEffect(() => {
		if (outOfSync !== null) setCursor(list, outOfSync);
	}, [outOfSync, list]);
};
