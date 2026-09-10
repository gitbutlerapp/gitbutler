import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

/**
 * Fold state for the upstream and History sections below the stacks.
 * Their rows share the applied list's cursor.
 */
export type GraphState = {
	/** The target header's fold: the commits incoming from the target. */
	incomingExpanded: boolean;
	historyExpanded: boolean;
	moreHistory: number;
	/** How many times more of each run was asked for, keyed by the run's newest commit id. */
	moreRuns: Record<string, number>;
};

const initialState = (): GraphState => ({
	incomingExpanded: false,
	historyExpanded: false,
	moreHistory: 0,
	moreRuns: {},
});

const graphSlice = createSlice({
	name: "graph",
	initialState,
	reducers: {
		toggleIncoming: (state) => {
			state.incomingExpanded = !state.incomingExpanded;
		},
		toggleHistory: (state) => {
			state.historyExpanded = !state.historyExpanded;
			if (!state.historyExpanded) state.moreHistory = 0;
		},
		showMoreHistory: (state) => {
			if (!state.historyExpanded) return;
			state.moreHistory += 1;
		},
		showMoreRun: (state, { payload: { runId } }: PayloadAction<{ runId: string }>) => {
			state.moreRuns[runId] = (state.moreRuns[runId] ?? 0) + 1;
		},
		foldRun: (state, { payload: { runId } }: PayloadAction<{ runId: string }>) => {
			delete state.moreRuns[runId];
		},
	},
	selectors: {
		selectGraphFolds: (state) => state,
	},
});

export const createInitialGraphState = (): GraphState => graphSlice.getInitialState();

export const graphReducers = {
	toggleHistory: (state: GraphState) => {
		graphSlice.caseReducers.toggleHistory(state);
	},
	showMoreHistory: (state: GraphState) => {
		graphSlice.caseReducers.showMoreHistory(state);
	},
	toggleIncoming: (state: GraphState) => {
		graphSlice.caseReducers.toggleIncoming(state);
	},
	showMoreRun: (state: GraphState, payload: { runId: string }) => {
		graphSlice.caseReducers.showMoreRun(state, graphSlice.actions.showMoreRun(payload));
	},
	foldRun: (state: GraphState, payload: { runId: string }) => {
		graphSlice.caseReducers.foldRun(state, graphSlice.actions.foldRun(payload));
	},
};

export const getGraphSelectors = <T>(selectState: (state: T) => GraphState) =>
	graphSlice.getSelectors(selectState);
