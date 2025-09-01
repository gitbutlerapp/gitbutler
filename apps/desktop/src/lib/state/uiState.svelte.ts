import { type SnapPositionName } from '$lib/floating/types';
import { InjectionToken } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { type Reactive } from '@gitbutler/shared/storeUtils';
import { isStr } from '@gitbutler/ui/utils/string';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';
import type { ThinkingLevel } from '$lib/codegen/types';
import type { PullRequest } from '$lib/forge/interface/types';
import type { StackDetails } from '$lib/stacks/stack';
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
			stackId: string | undefined;
			branchName: string | undefined;
			parentCommitId?: string;
	  }
	| {
			type: 'edit-commit-message';
			stackId: string | undefined;
			branchName: string;
			commitId: string;
	  }
	| {
			type: 'create-pr';
			stackId: string | undefined;
			branchName: string;
	  };

export type ProjectUiState = {
	exclusiveAction: ExclusiveAction | undefined;
	stackId: string | undefined;
	branchesSelection: BranchesSelection;
	showActions: boolean;
	branchesToPoll: string[];
	selectedClaudeSession: { stackId: string; head: string } | undefined;
	thinkingLevel: ThinkingLevel;
};

type GlobalModalType = 'commit-failed' | 'author-missing';
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

export type AuthorMissingModalState = BaseGlobalModalState & {
	type: 'author-missing';
	projectId: string;
	authorName: string | undefined;
	authorEmail: string | undefined;
};

export type GlobalModalState = CommitFailedModalState | AuthorMissingModalState;

export type GlobalUiState = {
	drawerHeight: number;
	stackWidth: number;
	detailsWidth: number;
	previewWidth: number;
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

	private scopesCache = {
		lanes: {} as Record<string, GlobalStore<any>>,
		projects: {} as Record<string, GlobalStore<any>>
	};

	/** Properties scoped to a specific stack. */
	readonly lane = this.buildScopedProps<StackState>(this.scopesCache.lanes, {
		selection: undefined,
		newCommitMessage: { title: '', description: '' }
	});

	/** Properties scoped to a specific project. */
	readonly project = this.buildScopedProps<ProjectUiState>(this.scopesCache.projects, {
		exclusiveAction: undefined,
		branchesSelection: {},
		stackId: undefined,
		showActions: false,
		branchesToPoll: [],
		selectedClaudeSession: undefined,
		thinkingLevel: 'normal'
	});

	/** Properties that are globally scoped. */
	readonly global = this.buildGlobalProps<GlobalUiState>({
		drawerHeight: 20,
		stackWidth: 22.5,
		detailsWidth: 32,
		previewWidth: 48,
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
			const result = this.getById(key, defaultValue);
			let mutableResult = $derived(result.current);
			props[key] = {
				set: (value: UiStateValue) => {
					mutableResult = value;
					this.update(key, value);
				},
				get current() {
					return mutableResult;
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
	private buildScopedProps<T extends DefaultConfig>(
		scopeCache: Record<string, GlobalStore<T>>,
		defaultConfig: T
	): (id: string) => GlobalStore<T> {
		return (id: string) => {
			if (id in scopeCache) {
				return scopeCache[id] as GlobalStore<T>;
			}
			const props: GlobalStore<DefaultConfig> = {};
			for (const [key, defaultValue] of Object.entries(defaultConfig)) {
				const result = this.getById(`${id}:${key}`, defaultValue);

				// We need a mutable value here for read/write consistency.
				let mutableResult = $derived(result.current);

				props[key] = {
					set: (value: UiStateValue) => {
						mutableResult = value;
						this.update(`${id}:${key}`, value);
					},
					get current() {
						return mutableResult;
					}
				};

				// If the value is an array of strings, we add methods to add/remove
				if (Array.isArray(mutableResult) && mutableResult.every(isStr)) {
					const result = mutableResult;
					(props[key] as GlobalProperty<string[]>).add = (...value: string[]) => {
						mutableResult = [...result, ...value.filter((v) => !result.includes(v))];
						this.update(`${id}:${key}`, mutableResult);
					};
					(props[key] as GlobalProperty<string[]>).remove = (value: string) => {
						mutableResult = result.filter((v) => v !== value);
						this.update(`${id}:${key}`, mutableResult);
					};
				}
			}
			scopeCache[id] = props as GlobalStore<T>;
			return scopeCache[id];
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
type UiStateValue =
	| string
	| string[]
	| number
	| boolean
	| { [property: string]: UiStateValue }
	| undefined;

/** Type held by the RTK entity adapter. */
type UiStateVariable = {
	id: string;
	value: UiStateValue;
};

/** Shape of the config expected by the build functions. */
type DefaultConfig = Record<string, UiStateValue>;

type ArrayPropertyMethods<T> = T extends string[]
	? {
			/** Will not add the value if it already exists in the array. */
			add(...value: string[]): void;
			/** Removes the value from the array. */
			remove(value: string): void;
		}
	: // eslint-disable-next-line @typescript-eslint/no-empty-object-type
		{};

/** Node type for global properties. */
export type GlobalProperty<T> = {
	set(value: T): void;
} & Reactive<T> &
	ArrayPropertyMethods<T>;

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

/**
 * Updates the currently selected stack and exclusive action if it is not in the provided list of stack IDs.
 */
export function updateStaleProjectState(
	uiState: UiState,
	projectId: string,
	stackIds: string[]
): void {
	const projectState = uiState.project(projectId);
	const selectedStack = projectState.stackId.current;
	if (selectedStack && !stackIds.includes(selectedStack)) {
		projectState.stackId.set(undefined);
	}

	const exclusiveAction = projectState.exclusiveAction.current;
	if (!exclusiveAction) return;

	switch (exclusiveAction.type) {
		case 'commit':
			if (exclusiveAction.stackId !== undefined && !stackIds.includes(exclusiveAction.stackId)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case 'edit-commit-message':
			if (exclusiveAction.stackId !== undefined && !stackIds.includes(exclusiveAction.stackId)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case 'create-pr':
			if (exclusiveAction.stackId !== undefined && !stackIds.includes(exclusiveAction.stackId)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
	}
}

function updateStackSelection(uiState: UiState, stackId: string, details: StackDetails): void {
	const laneState = uiState.lane(stackId);
	const selection = laneState.selection.current;
	const branches = details.branchDetails.map((branch) => branch.name);

	// If no selection, do nothing
	if (!selection) return;

	// Clear selection if the selected branch is not in the list of branches
	if (!branches.includes(selection.branchName)) {
		laneState.selection.set(undefined);
		return;
	}

	// If the selected branch exists and there is no commit selected, do nothing
	if (!selection.commitId) return;

	const selectedBranch = selection.branchName;
	const branchDetails = details.branchDetails.find((branch) => branch.name === selectedBranch);

	if (!branchDetails) {
		// Should not happen since we already checked the branch exists
		return;
	}

	const branchCommits = branchDetails.commits;
	const branchCommitIds = branchCommits.map((commit) => commit.id);

	// If the selected commit is not in the branch, clear the commit selection
	if (!selection.upstream && !branchCommitIds.includes(selection.commitId)) {
		laneState.selection.set({
			branchName: selection.branchName
		});

		return;
	}

	const upstreamCommits = branchDetails.upstreamCommits;
	const upstreamCommitIds = upstreamCommits.map((commit) => commit.id);

	// If the selection is for an upstream commit and the commit is not in the upstream commits, clear the selection
	if (selection.upstream && !upstreamCommitIds.includes(selection.commitId)) {
		laneState.selection.set({
			branchName: selection.branchName
		});

		return;
	}
}

/**
 * Updates the current stack state selection and exclusive action.
 */
export function updateStaleStackState(
	uiState: UiState,
	projectId: string,
	stackId: string,
	details: StackDetails
): void {
	updateStackSelection(uiState, stackId, details);

	const projectState = uiState.project(projectId);
	const exclusiveAction = projectState.exclusiveAction.current;
	if (!exclusiveAction) return;

	if (exclusiveAction.stackId === undefined || exclusiveAction.stackId !== stackId) {
		return;
	}

	const branches = details.branchDetails.map((branch) => branch.name);

	// If the exclusive action branch is not in the list of branches, clear the exclusive action
	if (exclusiveAction.branchName && !branches.includes(exclusiveAction.branchName)) {
		projectState.exclusiveAction.set(undefined);
		return;
	}

	const branchDetails = details.branchDetails.find(
		(branch) => branch.name === exclusiveAction.branchName
	);
	if (!branchDetails) {
		// Should not happen since we already checked the branch exists
		return;
	}

	const commitIds = branchDetails.commits.map((commit) => commit.id);

	switch (exclusiveAction.type) {
		case 'commit':
			if (exclusiveAction.parentCommitId && !commitIds.includes(exclusiveAction.parentCommitId)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case 'edit-commit-message':
			if (!commitIds.includes(exclusiveAction.commitId)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case 'create-pr':
			// Do nothing, alles gut
			break;
	}
}

export function updateStalePrSelection(uiState: UiState, projectId: string, prs: PullRequest[]) {
	const projectState = uiState.project(projectId);
	if (projectState.branchesSelection.current.prNumber === undefined) {
		return;
	}

	const prNumber = projectState.branchesSelection.current.prNumber;
	if (!prs.some((pr) => pr.number === prNumber)) {
		projectState.branchesSelection.set({});
	}
}
