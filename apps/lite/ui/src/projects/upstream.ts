import { operandEquals, operandIdentityKey, type Operand } from "#ui/operands.ts";
import {
	resolveNavigationIndexSelection,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

export type UpstreamState = {
	selection: Operand | null;
	/**
	 * Expanded shared-history segments, keyed by the segment's newest commit
	 * id. Expanding reveals the target commits between fork points, or — for
	 * the segment below the deepest fork point — pages of older target history.
	 */
	expandedSegments: Record<string, true>;
	/**
	 * Whether the incoming commits are folded away. Unlike the shared-history
	 * segments they show by default, so their toggle is stored inverted.
	 */
	incomingHidden: boolean;
};

const initialState = (): UpstreamState => ({
	selection: null,
	expandedSegments: {},
	incomingHidden: false,
});

const upstreamSlice = createSlice({
	name: "upstream",
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
		toggleSegment: (state, { payload: { segmentId } }: PayloadAction<{ segmentId: string }>) => {
			if (state.expandedSegments[segmentId]) delete state.expandedSegments[segmentId];
			else state.expandedSegments[segmentId] = true;
		},
		toggleIncoming: (state) => {
			state.incomingHidden = !state.incomingHidden;
		},
	},
	selectors: {
		/** The selection as stored, without resolving it against a navigation index. */
		selectPrimaryUpstreamSelection: (state) => state.selection,
		selectExpandedUpstreamSegments: (state) => state.expandedSegments,
		selectUpstreamIncomingHidden: (state) => state.incomingHidden,
		selectSelectionUpstream: (state, navigationIndex: NavigationIndex<Operand>) =>
			resolveNavigationIndexSelection(navigationIndex, state.selection, operandIdentityKey),
	},
});

export const createInitialUpstreamState = (): UpstreamState => upstreamSlice.getInitialState();

export const upstreamReducers = {
	select: (state: UpstreamState, payload: { selection: Operand | null }) => {
		upstreamSlice.caseReducers.select(state, upstreamSlice.actions.select(payload));
	},
	toggleSegment: (state: UpstreamState, payload: { segmentId: string }) => {
		upstreamSlice.caseReducers.toggleSegment(state, upstreamSlice.actions.toggleSegment(payload));
	},
	toggleIncoming: (state: UpstreamState) => {
		upstreamSlice.caseReducers.toggleIncoming(state);
	},
};

export const getUpstreamSelectors = <T>(selectState: (state: T) => UpstreamState) =>
	upstreamSlice.getSelectors(selectState);
