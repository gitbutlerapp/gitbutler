import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { createSelector, type EntityState } from '@reduxjs/toolkit';

export function createSelectNth<T>() {
	return createSelector(
		[(state: EntityState<T, number | string>) => state, (state_, index: number) => index],
		(state, index) => {
			if (state.ids.length > 0) {
				const id = state.ids[index];
				if (id) {
					const entity = state.entities[id];
					if (entity) {
						return entity;
					}
				}
			}
			return null;
		}
	);
}

export function createSelectByIds<T>() {
	return createSelector(
		[(state: EntityState<T, number | string>) => state, (state_, ids: string[]) => ids],
		(state, ids) => {
			return ids.map((id) => state.entities[id]).filter(isDefined);
		}
	);
}

/**
 * The main purpose of this function is to enable selecting e.g. the
 * parent of a branch, or commit.
 */
export function selectSelectNthAfterId<T>() {
	return createSelector(
		[(state: EntityState<T, number | string>) => state, (state_, id: string | number) => id],
		(state, id) => {
			if (state.ids.length > 0) {
				const index = state.ids.indexOf(id);
				if (index !== -1) {
					const nthId = state.ids[index + 1];
					if (nthId !== undefined) {
						return state.entities[nthId];
					}
				}
				if (id) {
					return state.entities[id];
				}
			}
			return null;
		}
	);
}

export function createSelectByPrefix<T>() {
	return createSelector(
		[(state: EntityState<T, string>) => state, (state_, prefix: string) => prefix],
		(state, prefix) =>
			state.ids
				.filter((id) => id.startsWith(prefix))
				.map((id) => state.entities[id])
				.filter(isDefined)
	);
}
