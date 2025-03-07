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
