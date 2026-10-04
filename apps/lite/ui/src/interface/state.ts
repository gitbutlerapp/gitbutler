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

/** What the details pane gives an overview of, when a mesh row has no diff of its own. */
export type MeshOverview =
	| { _tag: "Machine"; machine: string }
	/** With `machine`, the repo as that machine has it. */
	| { _tag: "Repo"; projectId: string; machine?: string };

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
		/** Every row starts folded, and loads what it shows only once unfolded. */
		unfolded: Record<string, true>;
		selection: string | null;
		overview: MeshOverview | null;
	};
};

const initialState: InterfaceState = {
	detailsFullWindow: false,
	diffFooterView: "feedback",
	dialog: { _tag: "None" },
	mesh: { grouping: "machines", unfolded: {}, selection: null, overview: null },
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
			if (state.mesh.unfolded[key]) delete state.mesh.unfolded[key];
			else state.mesh.unfolded[key] = true;
		},
		selectMeshRow: (
			state,
			{ payload: { key, overview } }: PayloadAction<{ key: string; overview: MeshOverview | null }>,
		) => {
			state.mesh.selection = key;
			state.mesh.overview = overview;
		},
	},
	selectors: {
		selectDiffFooterView: (state) => state.diffFooterView,
		selectDetailsFullWindow: (state) => state.detailsFullWindow,
		selectDialogState: (state) => state.dialog,
		selectMeshGrouping: (state) => state.mesh.grouping,
		selectMeshUnfolded: (state) => state.mesh.unfolded,
		selectMeshSelection: (state) => state.mesh.selection,
		selectMeshOverview: (state) => state.mesh.overview,
		selectIsMeshRowSelected: (state, key: string) => state.mesh.selection === key,
	},
});
