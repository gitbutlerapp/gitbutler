import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';

type DrawerPage = 'branch' | 'new-commit' | 'pr' | 'br' | 'branch' | undefined;

type CommitSelection = {
	branchName: string;
	commitId?: string;
	upstream?: boolean;
};

/**
 * Stateful properties for the UI, with redux backed fine-grained reactivity.
 */
export class UiState {
	private state = $state<EntityState<UiStateVariable, string>>(uiStateSlice.getInitialState());

	/** Properties scoped to a specific stack. */
	readonly stack = this.buildScopedProps({
		selection: undefined as CommitSelection | undefined
	});

	/** Properties scoped to a specific project. */
	readonly project = this.buildScopedProps({
		drawerPage: undefined as DrawerPage
	});

	/** Properties that are globally scoped. */
	readonly global = this.buildGlobalProps({
		drawerHeight: 20,
		leftWidth: 25,
		rightWidth: 25
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
		const props = {} as GlobalStore<DefaultConfig>;
		for (const [key, defaultValue] of Object.entries(param)) {
			props[key] = {
				get: () => this.getById(key, defaultValue),
				set: (val: UiStateValue) => this.update(key, val)
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
			const props = {} as GlobalStore<DefaultConfig>;
			for (const [key, defaultValue] of Object.entries(param)) {
				props[key] = {
					get: () => this.getById(`${id}:${key}`, defaultValue),
					set: (val: UiStateValue) => this.update(`${id}:${key}`, val)
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
type GlobalProperty<T> = {
	get(): Reactive<T>;
	set(value: T): void;
};

/** Type returned by the build function for global properties. */
type GlobalStore<T extends DefaultConfig> = {
	[K in keyof T]: GlobalProperty<T[K]>;
};
