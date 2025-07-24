import { type SnapPositionName } from '$lib/floating/types';
import { InjectionToken } from '@gitbutler/shared/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { type Reactive, type WritableReactive } from '@gitbutler/shared/storeUtils';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';
import type { RejectionReason } from '$lib/stacks/stackService.svelte';

export type StackSelection = {
	branchName: string;
	commitId?: string;
	upstream?: boolean;
};

export type NewCommitMessage = {
	title: string;
	description: string;
};

export type StackState = {
	selection: StackSelection | undefined;
	newCommitMessage: NewCommitMessage;
};

type BranchesSelection = {
	branchName?: string;
	commitId?: string;
	stackId?: string;
	remote?: string;
	hasLocal?: boolean;
	isTarget?: boolean;
	inWorkspace?: boolean;
	prNumber?: number;
};

export type ExclusiveAction =
	| {
			type: 'commit';
			stackId?: string;
			branchName?: string;
			parentCommitId?: string;
	  }
	| {
			type: 'edit-commit-message';
			commitId: string;
	  }
	| {
			type: 'create-pr';
			stackId?: string;
			branchName?: string;
	  };

export type ProjectUiState = {
	exclusiveAction: ExclusiveAction | undefined;
	stackId: string | undefined;
	branchesSelection: BranchesSelection;
	showActions: boolean;
};

type GlobalModalType = 'commit-failed';
type BaseGlobalModalState = {
	type: GlobalModalType;
};

export type CommitFailedModalState = BaseGlobalModalState & {
	type: 'commit-failed';
	projectId: string;
	targetBranchName: string;
	newCommitId: string | undefined;
	commitTitle: string | undefined;
	pathsToRejectedChanges: Record<string, RejectionReason>;
};

export type GlobalModalState = CommitFailedModalState;

export type GlobalUiState = {
	drawerHeight: number;
	stackWidth: number;
	detailsWidth: number;
	previewWidth: number;
	historySidebarWidth: number;
	branchesViewSidebarWidth: number;
	useFloatingBox: boolean;
	floatingBoxSize: {
		width: number;
		height: number;
	};
	floatingBoxPosition: SnapPositionName;
	unassignedSidebaFolded: boolean;
	useRuler: boolean;
	rulerCountValue: number;
	aiSuggestionsOnType: boolean;
	channel: string | undefined;
	draftBranchName: string | undefined;
	modal: GlobalModalState | undefined;
};

export const UI_STATE = new InjectionToken<UiState>('UiState');

/**
 * Stateful properties for the UI, with redux backed fine-grained reactivity.
 */
export class UiState {
	private state = $state.raw<EntityState<UiStateVariable, string>>(uiStateSlice.getInitialState());

	/** Properties scoped to a specific stack. */
	readonly stack = this.buildScopedProps<StackState>({
		selection: undefined,
		newCommitMessage: { title: '', description: '' }
	});

	/** Properties scoped to a specific project. */
	readonly project = this.buildScopedProps<ProjectUiState>({
		exclusiveAction: undefined,
		branchesSelection: {},
		stackId: undefined,
		showActions: false
	});

	/** Properties that are globally scoped. */
	readonly global = this.buildGlobalProps<GlobalUiState>({
		drawerHeight: 20,
		stackWidth: 22.5,
		detailsWidth: 32,
		previewWidth: 48,
		historySidebarWidth: 30,
		branchesViewSidebarWidth: 30,
		useFloatingBox: false,
		floatingBoxSize: {
			width: 640,
			height: 330
		},
		floatingBoxPosition: 'bottom-center',
		unassignedSidebaFolded: false,
		useRuler: true,
		rulerCountValue: 72,
		aiSuggestionsOnType: false,
		channel: undefined,
		draftBranchName: undefined,
		modal: undefined
	});

	constructor(
		reactiveState: Reactive<typeof this.state>,
		private dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		$effect(() => {
			this.state = reactiveState.current;
		});
	}

	private getById(id: string, defaultValue: UiStateValue): Reactive<UiStateValue> {
		const item = $derived(selectById(this.state, id));
		return reactive(() => (item?.value !== undefined ? item.value : defaultValue));
	}

	private update(id: string, value: UiStateValue) {
		this.dispatch(upsertOne({ id, value }));
	}

	/**
	 * Generate redux backed properties corresponding to the shape of the
	 * parameter value, with types corresponding to their default values.
	 */
	private buildGlobalProps<T extends DefaultConfig>(param: T): GlobalStore<T> {
		const props: GlobalStore<DefaultConfig> = {};
		for (const [key, defaultValue] of Object.entries(param)) {
			const current = this.getById(key, defaultValue);
			const boundUpdate = this.update.bind(this);
			props[key] = {
				get: () => current,
				set: (val: UiStateValue) => this.update(key, val),
				get current() {
					return current.current;
				},
				set current(value: UiStateValue) {
					boundUpdate(key, value);
				}
			};
		}
		return props as GlobalStore<T>;
	}

	/**
	 * Scoped props are the same as global, except they take an additional
	 * parameter for the key. This allows us to define values that are scoped
	 * to e.g. a projectId.
	 */
	private buildScopedProps<T extends DefaultConfig>(param: T): (id: string) => GlobalStore<T> {
		return (id: string) => {
			const props: GlobalStore<DefaultConfig> = {};
			for (const [key, defaultValue] of Object.entries(param)) {
				const current = this.getById(`${id}:${key}`, defaultValue);
				const boundUpdate = this.update.bind(this);
				props[key] = {
					get: () => current,
					set: (val: UiStateValue) => this.update(`${id}:${key}`, val),
					get current() {
						return current.current;
					},
					set current(value: UiStateValue) {
						boundUpdate(`${id}:${key}`, value);
					}
				};
			}
			return props as GlobalStore<T>;
		};
	}
}

export const uiStateAdapter = createEntityAdapter<UiStateVariable, string>({
	selectId: (item) => item.id
});

const { selectById } = uiStateAdapter.getSelectors();

export const uiStateSlice = createSlice({
	name: 'uiState',
	initialState: uiStateAdapter.getInitialState(),
	reducers: {
		upsertOne: uiStateAdapter.upsertOne
	},
	selectors: { selectById }
});

const { upsertOne } = uiStateSlice.actions;

/** Allowed types for property values. */
type UiStateValue = string | number | boolean | { [property: string]: UiStateValue } | undefined;

/** Type held by the RTK entity adapter. */
type UiStateVariable = {
	id: string;
	value: UiStateValue;
};

/** Shape of the config expected by the build functions. */
type DefaultConfig = Record<string, UiStateValue>;

/** Node type for global properties. */
export type GlobalProperty<T> = {
	get(): Reactive<T>;
	set(value: T): void;
} & WritableReactive<T>;

/** Type returned by the build function for global properties. */
type GlobalStore<T extends DefaultConfig> = {
	[K in keyof T]: GlobalProperty<T[K]>;
};

export function replaceBranchInExclusiveAction(
	action: ExclusiveAction,
	oldBranchName: string,
	branchName: string
): ExclusiveAction {
	switch (action.type) {
		case 'commit':
			if (action.branchName === oldBranchName) {
				return { ...action, branchName };
			}
			return action;
		case 'edit-commit-message':
			return action; // No change needed
		case 'create-pr':
			if (action.branchName === oldBranchName) {
				return { ...action, branchName };
			}
			return action;
	}
}

export function replaceBranchInStackSelection(
	selection: StackSelection,
	oldBranchName: string,
	branchName: string
): StackSelection {
	if (selection.branchName === oldBranchName) {
		return { ...selection, branchName };
	}
	return selection;
}
