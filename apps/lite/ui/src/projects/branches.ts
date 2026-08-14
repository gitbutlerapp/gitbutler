import type { BranchFilters } from "#ui/branch.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

export type BranchFilter = keyof BranchFilters;

/**
 * The branches tab's list configuration. Its cursor lives in the project's
 * cursor table (`cursors.branches`), with every other list's.
 */
export type BranchesState = {
	filters: BranchFilters;
	search: string;
	/** Branches with their commits unfolded, keyed by full ref name. */
	unfolded: Record<string, true>;
};

const initialState = (): BranchesState => ({
	filters: { showEmpty: false, onlyLocal: true, onlyStacks: false },
	search: "",
	unfolded: {},
});

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
		setSearch: (state, { payload: { search } }: PayloadAction<{ search: string }>) => {
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

export const createInitialBranchesState = (): BranchesState => branchesSlice.getInitialState();

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
	setSearch: (state: BranchesState, payload: { search: string }) => {
		branchesSlice.caseReducers.setSearch(state, branchesSlice.actions.setSearch(payload));
	},
	toggleFilter: (state: BranchesState, payload: { filter: BranchFilter }) => {
		branchesSlice.caseReducers.toggleFilter(state, branchesSlice.actions.toggleFilter(payload));
	},
};

export const getBranchesSelectors = <T>(selectState: (state: T) => BranchesState) =>
	branchesSlice.getSelectors(selectState);
