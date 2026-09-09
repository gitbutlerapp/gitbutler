import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

/**
 * The stacks graph's fold state: what the upstream section below the cards
 * shows. Its rows share the applied list's cursor.
 */
export type GraphState = {
	/** The target header's fold: the commits incoming from the target. */
	incomingExpanded: boolean;
	/** How many times more of each run was asked for, keyed by the run's newest commit id. */
	moreRuns: Record<string, number>;
};

const initialState = (): GraphState => ({
	incomingExpanded: false,
	moreRuns: {},
});

const graphSlice = createSlice({
	name: "graph",
	initialState,
	reducers: {
		toggleIncoming: (state) => {
			state.incomingExpanded = !state.incomingExpanded;
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
