import { createSelectByPrefix } from '$lib/state/customSelectors';
import { type Reactive, reactive } from '@gitbutler/shared/storeUtils';
import { type LineId } from '@gitbutler/ui/utils/diffParsing';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';

export class HunkSelection {
	private state = $state([]);
}

type HunkHeader = {
	oldStart: number;
	oldLines: number;
	newStart: number;
	newLines: number;
};

export type FullySelectedHunk = HunkHeader & {
	type: 'full';
};

export type PartiallySelectedHunk = HunkHeader & {
	type: 'partial';
	lines: LineId[];
};

/**
 * Representation of visually selected hunk.
 */
export type SelectedHunk = FullySelectedHunk | PartiallySelectedHunk;

type FileHeader = {
	path: string;
	pathBytes: number[];
};

export type FullySelectedFile = FileHeader & {
	type: 'full';
};

export type PartiallySelectedFile = FileHeader & {
	type: 'partial';
	hunks: SelectedHunk[];
};

/**
 * Representation of visually selected file.
 */
export type SelectedFile = FullySelectedFile | PartiallySelectedFile;

export const changeSelectionAdapter = createEntityAdapter<SelectedFile, SelectedFile['path']>({
	selectId: (change) => change.path,
	sortComparer: (a, b) => a.path.localeCompare(b.path)
});

const { selectById, selectAll } = changeSelectionAdapter.getSelectors();
const selectByPrefix = createSelectByPrefix<SelectedFile>();

export const changeSelectionSlice = createSlice({
	name: 'changeSelection',
	initialState: changeSelectionAdapter.getInitialState(),
	reducers: {
		addOne: changeSelectionAdapter.addOne,
		removeOne: changeSelectionAdapter.removeOne,
		removeMany: changeSelectionAdapter.removeMany,
		removeAll: changeSelectionAdapter.removeAll,
		upsertOne: changeSelectionAdapter.upsertOne
	},
	selectors: { selectById, selectAll }
});

const { addOne, removeOne, removeMany, removeAll, upsertOne } = changeSelectionSlice.actions;

export class ChangeSelectionService {
	/** The change selection slice of the full redux state. */
	private state = $state<EntityState<SelectedFile, string>>(changeSelectionSlice.getInitialState());

	constructor(
		reactiveState: Reactive<typeof this.state>,
		private dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		$effect(() => {
			this.state = reactiveState.current;
		});
	}

	list(): Reactive<SelectedFile[]> {
		const selected = $derived(selectAll(this.state));
		return reactive(() => selected);
	}

	getById(path: string): Reactive<SelectedFile | undefined> {
		const selected = $derived(selectById(this.state, path));
		return reactive(() => selected);
	}

	getByPrefix(path: string): Reactive<SelectedFile[]> {
		const selected = $derived(selectByPrefix(this.state, path));
		return reactive(() => selected);
	}

	add(file: SelectedFile) {
		this.dispatch(addOne(file));
	}

	update(file: SelectedFile) {
		this.dispatch(upsertOne(file));
	}

	remove(path: string) {
		this.dispatch(removeOne(path));
	}

	/** Clears any selected items that are not in `paths`.  */
	retain(paths: string[] | undefined) {
		if (paths === undefined) {
			this.dispatch(removeAll());
			return;
		}
		const selection = $derived(selectAll(this.state));
		const expired = [];
		for (const change of selection) {
			if (!paths.includes(change.path)) {
				expired.push(change.path);
			}
		}
		if (expired.length > 0) {
			this.dispatch(removeMany(expired));
		}
	}
}
