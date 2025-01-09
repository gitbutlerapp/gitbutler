import { ApiError, type Loadable, type LoadableData } from '$lib/network/types';
import type { EntityId, EntityAdapter, EntityState } from '@reduxjs/toolkit';

export function isFound<T>(loadable?: Loadable<T>): loadable is {
	status: 'found';
	value: T;
} {
	return loadable?.status === 'found';
}

export function errorToLoadable<T, Id>(error: unknown, id: Id): LoadableData<T, Id> {
	if (error instanceof Error) {
		if (error instanceof ApiError && error.response.status === 404) {
			return { status: 'not-found', id };
		}

		return { status: 'error', id, error };
	}

	return { status: 'error', id, error: new Error(String(error)) };
}

export function loadableUpsert<T, Id extends EntityId>(
	adapter: EntityAdapter<LoadableData<T, Id>, Id>
) {
	return (
		state: EntityState<LoadableData<T, Id>, Id>,
		action: { payload: LoadableData<T, Id> }
	) => {
		loadableUpsertMany(adapter)(state, { payload: [action.payload] });
	};
}

export function loadableUpsertMany<T, Id extends EntityId>(
	adapter: EntityAdapter<LoadableData<T, Id>, Id>
) {
	return (
		state: EntityState<LoadableData<T, Id>, Id>,
		action: { payload: LoadableData<T, Id>[] }
	) => {
		const values = action.payload.map((payload) => {
			const value = state.entities[payload.id];
			if (value === undefined) {
				return payload;
			}

			if (!(value.status === 'found' && payload.status === 'found')) {
				return payload;
			}

			const newValue: LoadableData<T, Id> = {
				status: 'found',
				id: payload.id,
				value: { ...value, ...payload.value }
			};

			return newValue;
		});

		adapter.setMany(state, values);
	};
}
