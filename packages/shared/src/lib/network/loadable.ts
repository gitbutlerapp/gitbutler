import { ApiError, type Loadable, type LoadableData } from '$lib/network/types';
import type { EntityId, EntityAdapter, EntityState } from '@reduxjs/toolkit';

export function isFound<T>(loadable?: Loadable<T>): loadable is {
	status: 'found';
	value: T;
} {
	return loadable?.status === 'found';
}

export function isError<T>(loadable?: Loadable<T>): loadable is { status: 'error'; error: Error } {
	return loadable?.status === 'error';
}

export function isNotFound<T>(loadable?: Loadable<T>): loadable is { status: 'not-found' } {
	return loadable?.status === 'not-found';
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
			const entity = state.entities[payload.id];
			if (entity === undefined) {
				return payload;
			}

			if (!(entity.status === 'found' && payload.status === 'found')) {
				return payload;
			}

			let merged: T;

			const unmergableTypes = ['string', 'number', 'boolean', 'bigint', 'symbol', 'undefined'];

			if (
				unmergableTypes.includes(typeof entity.value) ||
				unmergableTypes.includes(typeof payload.value)
			) {
				merged = payload.value;
			} else if (Array.isArray(entity.value) || Array.isArray(payload.value)) {
				merged = payload.value;
			} else {
				merged = { ...entity.value, ...payload.value };
			}

			const newValue: LoadableData<T, Id> = {
				status: 'found',
				id: payload.id,
				value: merged
			};

			return newValue;
		});

		adapter.setMany(state, values);
	};
}

export function and<T>(
	a: Loadable<unknown> | undefined,
	b: Loadable<T> | undefined
): Loadable<T> | undefined {
	if (isFound(a)) {
		return b;
	} else {
		return a;
	}
}

export function dig<T, R>(
	loadable: Loadable<T> | undefined,
	digger: (current: T) => R
): R | undefined {
	if (isFound(loadable)) {
		return digger(loadable.value);
	}
	return undefined;
}

export function compose<A, B>(
	a: Loadable<A> | undefined,
	b: Loadable<B> | undefined
): Loadable<[A, B]> {
	if (isFound(a) && isFound(b)) {
		return { status: 'found', value: [a.value, b.value] };
	}

	const failureStates = [isError, isNotFound];
	for (const state of failureStates) {
		if (state(a)) {
			return a;
		}
		if (state(b)) {
			return b;
		}
	}

	return { status: 'loading' };
}
