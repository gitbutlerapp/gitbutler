import { reactive, type Reactive, type WritableReactive } from '@gitbutler/shared/storeUtils';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';
import storage from 'redux-persist/lib/storage';
export type DrawerPage = 'branch' | 'new-commit' | 'review' | undefined;

export const uiStatePersistConfig = {
	key: 'uiState',
	storage: storage
};

export type StackSelection = {
	branchName: string;
	commitId?: string;
	upstream?: boolean;
};

export type StackState = {
	selection: StackSelection | undefined;
};

type BranchesSelection = {
	branchName?: string;
	commitId?: string;
	stackId?: string;
	remote?: string;
	prNumber?: number;
};

export type ProjectUiState = {
	drawerPage: DrawerPage;
	drawerFullScreen: boolean;
	commitTitle: string;
	commitDescription: string;
	branchesSelection: BranchesSelection;
};

export type GlobalUiState = {
	drawerHeight: number;
	leftWidth: number;
	stacksViewWidth: number;
	drawerSplitViewWidth: number;
	useRichText: boolean;
	useRuler: boolean;
	rulerCountValue: number;
	wrapTextByRuler: boolean;
	aiSuggestionsOnType: boolean;
	selectedTip: number | undefined;
	channel: string | undefined;
	draftBranchName: string | undefined;
};

/**
 * Stateful properties for the UI, with redux backed fine-grained reactivity.
 */
export class UiState {
	private state = $state<EntityState<UiStateVariable, string>>(uiStateSlice.getInitialState());

	/** Properties scoped to a specific stack. */
	readonly stack = this.buildScopedProps<StackState>({
		selection: undefined
	});

	/** Properties scoped to a specific project. */
	readonly project = this.buildScopedProps<ProjectUiState>({
		drawerPage: undefined,
		drawerFullScreen: false,
		commitTitle: '',
		commitDescription: '',
		branchesSelection: {}
	});

	/** Properties that are globally scoped. */
	readonly global = this.buildGlobalProps<GlobalUiState>({
		drawerHeight: 20,
		leftWidth: 17.5,
		stacksViewWidth: 21.25,
		drawerSplitViewWidth: 20,
		useRichText: false,
		useRuler: false,
		rulerCountValue: 72,
		wrapTextByRuler: false,
		aiSuggestionsOnType: false,
		selectedTip: undefined,
		channel: undefined,
		draftBranchName: undefined
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
type UiStateValue =
	| string
	| number
	| boolean
	| Record<string, string | number | boolean>
	| undefined;

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
