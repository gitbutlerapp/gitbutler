import type { BranchFilters } from "#ui/branch.ts";
import {
	branchOperand,
	commitOperand,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type Operand,
} from "#ui/operands.ts";
import {
	resolveNavigationIndexSelection,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

export type BranchFilter = keyof BranchFilters;

export type BranchesState = {
	selection: Operand | null;
	filters: BranchFilters;
	search: string;
	/** Branches with their commits unfolded, keyed by full ref name. */
	unfolded: Record<string, true>;
};

const initialState = (): BranchesState => ({
	selection: null,
	filters: { showEmpty: false, onlyLocal: true, onlyStacks: false },
	search: "",
	unfolded: {},
});

const branchesSlice = createSlice({
	name: "branches",
	initialState,
	reducers: {
		select: (state, { payload: { selection } }: PayloadAction<{ selection: Operand | null }>) => {
			if (
				selection === null
					? state.selection === null
					: state.selection !== null && operandEquals(state.selection, selection)
			)
				return;

			state.selection = selection;
		},
		updateRewrittenBranchReferences: (
			state,
			{
				payload: { oldBranch, newBranch },
			}: PayloadAction<{ oldBranch: BranchOperand; newBranch: BranchOperand }>,
		) => {
			const oldBranchOperand = branchOperand(oldBranch);
			if (state.selection?._tag === "Branch" && operandEquals(state.selection, oldBranchOperand))
				state.selection = branchOperand(newBranch);
		},
		updateRewrittenCommitReferences: (
			state,
			{ payload: { replacedCommits } }: PayloadAction<{ replacedCommits: Record<string, string> }>,
		) => {
			if (state.selection?._tag !== "Commit") return;

			const newId = replacedCommits[state.selection.commitId];
			if (newId !== undefined)
				state.selection = commitOperand({ commitId: newId, changeId: state.selection.changeId });
		},
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
		/** The selection as stored, without resolving it against a navigation index. */
		selectPrimaryBranchesSelection: (state) => state.selection,
		selectSelectionBranches: (state, navigationIndex: NavigationIndex<Operand>) =>
			resolveNavigationIndexSelection(navigationIndex, state.selection, operandIdentityKey),
	},
});

export const createInitialBranchesState = (): BranchesState => branchesSlice.getInitialState();

export const branchesReducers = {
	select: (state: BranchesState, payload: { selection: Operand | null }) => {
		branchesSlice.caseReducers.select(state, branchesSlice.actions.select(payload));
	},
	updateRewrittenBranchReferences: (
		state: BranchesState,
		payload: { oldBranch: BranchOperand; newBranch: BranchOperand },
	) => {
		branchesSlice.caseReducers.updateRewrittenBranchReferences(
			state,
			branchesSlice.actions.updateRewrittenBranchReferences(payload),
		);
	},
	updateRewrittenCommitReferences: (
		state: BranchesState,
		payload: { replacedCommits: Record<string, string> },
	) => {
		branchesSlice.caseReducers.updateRewrittenCommitReferences(
			state,
			branchesSlice.actions.updateRewrittenCommitReferences(payload),
		);
	},
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
