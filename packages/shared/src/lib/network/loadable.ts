import { ApiError, toSerializable, type Loadable, type LoadableData } from '$lib/network/types';
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

		return { status: 'error', id, error: toSerializable(error) };
	}

	return { status: 'error', id, error: toSerializable(error) };
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
				merged = { ...entity.value };

				for (const [key, value] of Object.entries(payload.value as object)) {
					if (value !== undefined && value !== null) {
						// @ts-expect-error This is fine
						merged[key] = value;
					}
				}
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
	loadables: [...(Loadable<unknown> | undefined)[], Loadable<T> | undefined]
): Loadable<T> | undefined {
	if (loadables.length <= 1) {
		return loadables[0] as Loadable<T> | undefined;
	}

	if (isFound(loadables[0])) {
		return and(
			loadables.slice(1) as [...(Loadable<unknown> | undefined)[], Loadable<T> | undefined]
		);
	} else {
		return loadables[0];
	}
}

export function map<T, R>(
	loadable: Loadable<T> | undefined,
	digger: (current: T) => R
): R | undefined {
	if (isFound(loadable)) {
		return digger(loadable.value);
	}
	return undefined;
}

export function mapL<T, R>(
	loadable: Loadable<T> | undefined,
	digger: (current: T) => R
): Loadable<R> {
	if (isFound(loadable)) {
		return { status: 'found', value: digger(loadable.value) };
	}
	return loadable as Loadable<R>;
}

export function combine<A extends [...unknown[]]>(loadables: {
	[K in keyof A]: Loadable<A[K]> | undefined;
}): Loadable<A> {
	if (loadables.every((loadable) => isFound(loadable))) {
		return {
			status: 'found',
			// @ts-expect-error I'm sure this could be typed propperly, but this is fine
			value: loadables.map((loadable) => loadable.value)
		};
	}

	const failureStates = [isError, isNotFound];
	for (const state of failureStates) {
		for (const loadable of loadables) {
			if (state(loadable)) {
				return loadable;
			}
		}
	}

	return { status: 'loading' };
}
