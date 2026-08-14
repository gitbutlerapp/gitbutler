import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

/**
 * The upstream tab's list configuration. Its cursor lives in the project's
 * cursor table (`cursors.upstream`), with every other list's.
 */
export type UpstreamState = {
	/**
	 * Expanded shared-history segments, keyed by the segment's newest commit
	 * id. Expanding reveals the target commits between two fork points.
	 */
	expandedSegments: Record<string, true>;
};

const initialState = (): UpstreamState => ({
	expandedSegments: {},
});

const upstreamSlice = createSlice({
	name: "upstream",
	initialState,
	reducers: {
		toggleSegment: (state, { payload: { segmentId } }: PayloadAction<{ segmentId: string }>) => {
			if (state.expandedSegments[segmentId]) delete state.expandedSegments[segmentId];
			else state.expandedSegments[segmentId] = true;
		},
	},
	selectors: {
		selectExpandedUpstreamSegments: (state) => state.expandedSegments,
	},
});

export const createInitialUpstreamState = (): UpstreamState => upstreamSlice.getInitialState();

export const upstreamReducers = {
	toggleSegment: (state: UpstreamState, payload: { segmentId: string }) => {
		upstreamSlice.caseReducers.toggleSegment(state, upstreamSlice.actions.toggleSegment(payload));
	},
};

export const getUpstreamSelectors = <T>(selectState: (state: T) => UpstreamState) =>
	upstreamSlice.getSelectors(selectState);
