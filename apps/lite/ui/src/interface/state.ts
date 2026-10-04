import type { SettingsPageKey } from "#ui/routes/project/$id/workspace/Settings/pages.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "OperationsLogPicker" }
	| { _tag: "ProjectPicker" }
	| { _tag: "Settings"; page?: SettingsPageKey }
	/** The update-from-remote flow, for the applied branch named by full ref. */
	| { _tag: "UpdateFromRemote"; branchRef: string };

export type MeshGrouping = "machines" | "repos";

type InterfaceState = {
	detailsFullWindow: boolean;
	diffFooterView: "feedback" | "dadJokes";
	dialog: Dialog;
	/**
	 * The mesh sidebar's view, kept here rather than per project since it spans them all and
	 * opening another project from it shouldn't reset it.
	 */
	mesh: {
		grouping: MeshGrouping;
		/** Rows toggled from how they start: most start unfolded, branches folded. */
		toggled: Record<string, true>;
		selection: string | null;
	};
};

const initialState: InterfaceState = {
	detailsFullWindow: false,
	diffFooterView: "feedback",
	dialog: { _tag: "None" },
	mesh: { grouping: "machines", toggled: {}, selection: null },
};

export const interfaceSlice = createSlice({
	name: "interface",
	initialState,
	reducers: {
		toggleDiffFooterView: (state) => {
			state.diffFooterView = state.diffFooterView === "dadJokes" ? "feedback" : "dadJokes";
		},
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
		setMeshGrouping: (
			state,
			{ payload: { grouping } }: PayloadAction<{ grouping: MeshGrouping }>,
		) => {
			state.mesh.grouping = grouping;
		},
		toggleMeshRow: (state, { payload: { key } }: PayloadAction<{ key: string }>) => {
			if (state.mesh.toggled[key]) delete state.mesh.toggled[key];
			else state.mesh.toggled[key] = true;
		},
		selectMeshRow: (state, { payload: { key } }: PayloadAction<{ key: string }>) => {
			state.mesh.selection = key;
		},
	},
	selectors: {
		selectDiffFooterView: (state) => state.diffFooterView,
		selectDetailsFullWindow: (state) => state.detailsFullWindow,
		selectDialogState: (state) => state.dialog,
		selectMeshGrouping: (state) => state.mesh.grouping,
		selectMeshToggled: (state) => state.mesh.toggled,
		selectMeshSelection: (state) => state.mesh.selection,
		selectIsMeshRowSelected: (state, key: string) => state.mesh.selection === key,
	},
});
