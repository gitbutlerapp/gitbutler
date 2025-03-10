import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import {
	createEntityAdapter,
	createSlice,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';

type StateKey = 'drawer';

type DrawerPage = 'commit' | 'pr' | 'br' | undefined;

type UiStateVariable = {
	id: string;
	value: string | number | boolean | undefined;
};

export class UiStateService {
	private state = $state<EntityState<UiStateVariable, string>>(uiStateSlice.getInitialState());

	constructor(
		reactiveState: Reactive<typeof this.state>,
		private dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		$effect(() => {
			console.log(reactiveState.current);
			this.state = reactiveState.current;
		});
	}

	getById(id: StateKey): Reactive<UiStateVariable | undefined> {
		const selected = $derived(selectById(this.state, id));
		return reactive(() => selected);
	}

	update(id: StateKey, value: string | number | boolean | undefined) {
		this.dispatch(upsertOne({ id, value }));
	}

	getDrawerPage(): Reactive<DrawerPage> {
		const state = $derived(this.getById('drawer'));
		return reactive(() => state.current?.value as DrawerPage);
	}
}

export const uiStateVariableAdapter = createEntityAdapter<UiStateVariable, string>({
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
