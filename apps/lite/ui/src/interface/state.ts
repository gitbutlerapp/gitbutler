import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "OperationsLogPicker" }
	| { _tag: "ProjectPicker" }
	| { _tag: "Settings" }
	/** The update-from-remote flow, for the applied branch named by full ref. */
	| { _tag: "UpdateFromRemote"; branchRef: string };

type InterfaceState = {
	detailsFullWindow: boolean;
	dialog: Dialog;
};

const initialState: InterfaceState = {
	detailsFullWindow: false,
	dialog: { _tag: "None" },
};

export const interfaceSlice = createSlice({
	name: "interface",
	initialState,
	reducers: {
		setDetailsFullWindow: (
			state,
			{ payload: { fullWindow } }: PayloadAction<{ fullWindow: boolean }>,
		) => {
			state.detailsFullWindow = fullWindow;
		},
		toggleDetailsFullWindow: (state) => {
			state.detailsFullWindow = !state.detailsFullWindow;
		},
		openDialog: (state, { payload: { dialog } }: PayloadAction<{ dialog: Dialog }>) => {
			state.dialog = dialog;
		},
		closeDialog: (state) => {
			state.dialog = { _tag: "None" };
		},
	},
	selectors: {
		selectDetailsFullWindow: (state) => state.detailsFullWindow,
		selectDialogState: (state) => state.dialog,
	},
});
