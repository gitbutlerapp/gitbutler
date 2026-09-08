import type { BranchFilters } from "#ui/branch.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

export type BranchFilter = keyof BranchFilters;

/**
 * The branches tab's list configuration. Its cursor lives in the project's
 * cursor table (`cursors.branches`), with every other list's.
 */
export type BranchesState = {
	filters: BranchFilters;
	/**
	 * The filter query, or `null` while the filter is closed — the list's header
	 * stands in its place then, as the file lists' filters do.
	 */
	search: string | null;
	/** Branches with their commits unfolded, keyed by full ref name. */
	unfolded: Record<string, true>;
};

/**
 * Nothing narrows the list until the user opts in: hiding remote-only
 * branches by default helped only the few repositories with hundreds of them,
 * and made everyone else's remote branches look missing.
 */
const defaultFilters = (): BranchFilters => ({
	showEmpty: false,
	onlyLocal: false,
	onlyStacks: false,
});

const initialState = (): BranchesState => ({
	filters: defaultFilters(),
	search: null,
	unfolded: {},
});

const filtersStorageKeyPrefix = "branch_filters:v1:";

const isBranchFilters = (value: unknown): value is BranchFilters =>
	typeof value === "object" &&
	value !== null &&
	(["showEmpty", "onlyLocal", "onlyStacks"] as const).every(
		(key) => typeof (value as Record<string, unknown>)[key] === "boolean",
	);

/**
 * The filters each project was last left with, by project id, so a reload
 * keeps the list narrowed the way the user set it. Storage can throw or hold
 * junk; either reads as a project at its defaults.
 */
export const readStoredBranchFilters = (): Record<string, BranchFilters> => {
	const filtersByProject: Record<string, BranchFilters> = {};
	try {
		for (let index = 0; index < window.localStorage.length; index++) {
			const key = window.localStorage.key(index);
			if (key === null || !key.startsWith(filtersStorageKeyPrefix)) continue;
			try {
				const stored: unknown = JSON.parse(window.localStorage.getItem(key) ?? "null");
				if (isBranchFilters(stored)) {
					const { showEmpty, onlyLocal, onlyStacks } = stored;
					filtersByProject[key.slice(filtersStorageKeyPrefix.length)] = {
						showEmpty,
						onlyLocal,
						onlyStacks,
					};
				}
			} catch {
				// One unparseable entry should not cost the other projects theirs.
			}
		}
	} catch {
		// Storage disabled or partitioned: every project starts at its defaults.
	}
	return filtersByProject;
};

export const writeStoredBranchFilters = (projectId: string, filters: BranchFilters): void => {
	try {
		window.localStorage.setItem(filtersStorageKeyPrefix + projectId, JSON.stringify(filters));
	} catch {
		// The in-memory copy still serves this session.
	}
};

const branchesSlice = createSlice({
	name: "branches",
	initialState,
	reducers: {
		toggleUnfolded: (state, { payload: { branchRef } }: PayloadAction<{ branchRef: string }>) => {
			if (state.unfolded[branchRef]) delete state.unfolded[branchRef];
			else state.unfolded[branchRef] = true;
		},
		/**
		 * Unfolds or folds several branches at once, for acting on a whole stack.
		 * Toggling each of them instead would invert a partly unfolded stack rather
		 * than bring it to one state.
		 */
		setUnfolded: (
			state,
			{
				payload: { branchRefs, unfolded },
			}: PayloadAction<{ branchRefs: Array<string>; unfolded: boolean }>,
		) => {
			for (const branchRef of branchRefs) {
				if (unfolded) state.unfolded[branchRef] = true;
				else delete state.unfolded[branchRef];
			}
		},
		setSearch: (state, { payload: { search } }: PayloadAction<{ search: string | null }>) => {
			if (state.search === search) return;

			state.search = search;
		},
		toggleFilter: (state, { payload: { filter } }: PayloadAction<{ filter: BranchFilter }>) => {
			state.filters[filter] = !state.filters[filter];
		},
	},
	selectors: {
		selectBranchFilters: (state) => state.filters,
		selectBranchSearch: (state) => state.search,
		selectUnfoldedBranches: (state) => state.unfolded,
		selectBranchUnfolded: (state, branchRef: string) => state.unfolded[branchRef] === true,
	},
});

export const createInitialBranchesState = (
	filters: BranchFilters = defaultFilters(),
): BranchesState => ({ ...branchesSlice.getInitialState(), filters });

export const branchesReducers = {
	toggleUnfolded: (state: BranchesState, payload: { branchRef: string }) => {
		branchesSlice.caseReducers.toggleUnfolded(state, branchesSlice.actions.toggleUnfolded(payload));
	},
	setUnfolded: (
		state: BranchesState,
		payload: { branchRefs: Array<string>; unfolded: boolean },
	) => {
		branchesSlice.caseReducers.setUnfolded(state, branchesSlice.actions.setUnfolded(payload));
	},
	setSearch: (state: BranchesState, payload: { search: string | null }) => {
		branchesSlice.caseReducers.setSearch(state, branchesSlice.actions.setSearch(payload));
	},
	toggleFilter: (state: BranchesState, payload: { filter: BranchFilter }) => {
		branchesSlice.caseReducers.toggleFilter(state, branchesSlice.actions.toggleFilter(payload));
	},
};

export const getBranchesSelectors = <T>(selectState: (state: T) => BranchesState) =>
	branchesSlice.getSelectors(selectState);
