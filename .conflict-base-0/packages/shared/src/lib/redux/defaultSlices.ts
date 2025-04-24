import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import {
	createEntityAdapter,
	createSlice,
	type EntityId,
	type EntityState,
	type Reducer,
	type EntitySelectors,
	type Action
} from '@reduxjs/toolkit';
import type { LoadableData } from '$lib/network/types';

type LoadableTable<T extends LoadableData<unknown, EntityId>> = {
	reducer: Reducer<EntityState<T, T['id']>>;
	selectors: EntitySelectors<T, EntityState<T, T['id']>, T['id']>;
	addOne: (value: T) => Action;
	addMany: (values: T[]) => Action;
	removeOne: (value: T['id']) => Action;
	removeMany: (values: T['id'][]) => Action;
	upsertOne: (value: T) => Action;
	upsertMany: (values: T[]) => Action;
};

export function buildLoadableTable<T extends LoadableData<unknown, EntityId>>(
	name: string
): LoadableTable<T> {
	const adapter = createEntityAdapter<T, T['id']>({
		selectId: (t: T) => t.id
	});

	const slice = createSlice({
		name: name,
		initialState: adapter.getInitialState(),
		reducers: {
			// @ts-expect-error generics + redux = sad
			addOne: adapter.addOne,
			// @ts-expect-error generics + redux = sad
			addMany: adapter.addMany,
			// @ts-expect-error generics + redux = sad
			removeOne: adapter.removeOne,
			// @ts-expect-error generics + redux = sad
			removeMany: adapter.removeMany,
			// @ts-expect-error generics + redux = sad
			upsertOne: loadableUpsert(adapter),
			// @ts-expect-error generics + redux = sad
			upsertMany: loadableUpsertMany(adapter)
		}
	});

	return {
		reducer: slice.reducer,
		selectors: adapter.getSelectors(),
		addOne: slice.actions.addOne,
		addMany: slice.actions.addMany,
		removeOne: slice.actions.removeOne,
		removeMany: slice.actions.removeMany,
		upsertOne: slice.actions.upsertOne,
		upsertMany: slice.actions.upsertMany
	} as LoadableTable<T>;
}
