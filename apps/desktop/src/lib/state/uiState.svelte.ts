import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';

type StateKey = 'drawer' | 'drawerWidth' | 'sideBarOpen';

type DrawerPage = 'commit' | 'pr' | 'br' | undefined;

interface UiStateVariableBase {
	id: StateKey;
	value: string | number | boolean | undefined;
}

interface UiStateDrawerPage extends UiStateVariableBase {
	id: 'drawer';
	value: DrawerPage;
}

interface UiStateDrawerWidth extends UiStateVariableBase {
	id: 'drawerWidth';
	value: number;
}

interface UiStateSideBarOpen extends UiStateVariableBase {
	id: 'sideBarOpen';
	value: boolean;
}

type UiStateVariable = UiStateDrawerPage | UiStateDrawerWidth | UiStateSideBarOpen;

type UiStateVariableForKey<T extends StateKey> = Extract<UiStateVariable, { id: T }>;

type UiStateValueForKey<T extends StateKey> = UiStateVariableForKey<T>['value'];

export class UiStateService {
	private state = $state<EntityState<UiStateVariable, StateKey>>(uiStateSlice.getInitialState());

	constructor(
		reactiveState: Reactive<typeof this.state>,
		private dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		$effect(() => {
			this.state = reactiveState.current;
		});
	}

	getById<T extends StateKey>(id: T): Reactive<UiStateVariableForKey<T> | undefined> {
		const selected = $derived(selectById(this.state, id));
		return reactive(() =>
			selected?.id === id ? (selected as UiStateVariableForKey<T>) : undefined
		);
	}

	update<T extends StateKey>(id: T, value: UiStateValueForKey<T>): void {
		this.dispatch(upsertOne({ id, value } as UiStateVariable));
	}

	getDrawerPage(): Reactive<DrawerPage> {
		const state = $derived(this.getById('drawer'));
		return reactive(() => state.current?.value);
	}
}

export const uiStateVariableAdapter = createEntityAdapter<UiStateVariable, StateKey>({
	selectId: (item) => item.id
});

const { selectById } = uiStateVariableAdapter.getSelectors();

export const uiStateSlice = createSlice({
	name: 'uiState',
	initialState: uiStateVariableAdapter.getInitialState(),
	reducers: {
		upsertOne: uiStateVariableAdapter.upsertOne
	},
	selectors: { selectById }
});

const { upsertOne } = uiStateSlice.actions;
