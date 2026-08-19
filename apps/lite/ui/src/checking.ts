/**
 * @file Generalised logic for consistent shift+clicking across groups of checkboxes.
 */

import type { AddressSpace } from "./workspace/address-space.ts";

type RangeResolver<T> = (range: { anchor: T; target: T }) => Set<T> | null;

export const addressSpaceRange =
	<T, K>({
		addressSpace: navidx,
		getKey,
		filterMap,
	}: {
		addressSpace: AddressSpace<T>;
		getKey: (id: K) => string;
		filterMap: (item: T) => K | null;
	}): RangeResolver<K> =>
	({ anchor, target }) => {
		const anchorIndex = navidx.indexByKey.get(getKey(anchor));
		if (anchorIndex === undefined) return null;

		const targetIndex = navidx.indexByKey.get(getKey(target));
		if (targetIndex === undefined) return null;

		const start = Math.min(anchorIndex, targetIndex);
		const end = Math.max(anchorIndex, targetIndex);

		return new Set(
			navidx.items.slice(start, end + 1).flatMap((item) => {
				const id = filterMap(item);
				return id === null ? [] : [id];
			}),
		);
	};

type RangeState<T> = {
	checked: Set<T>;
	rangeAnchor: T | null;
	rangeEnd: T | null;
};

type CheckEvent<T> = { item: T; shiftKey: boolean };

/**
 * Resolve a checking range given a generic resolver, previous state, and an event. This function
 * strictly compares the generic type. See also {@link addressSpaceRange}.
 */
export const checkedRange =
	<T>(resolveRange: RangeResolver<T>) =>
	(state: RangeState<T>) =>
	(evt: CheckEvent<T>): RangeState<T> => {
		range: if (evt.shiftKey && state.checked.size > 0 && state.rangeAnchor !== null) {
			const activeRange = resolveRange({
				anchor: state.rangeAnchor,
				target: evt.item,
			});
			if (!activeRange) break range;

			const previousActiveRange =
				(state.rangeEnd !== null
					? resolveRange({ anchor: state.rangeAnchor, target: state.rangeEnd })
					: null) ?? new Set([state.rangeAnchor]);

			const deactivatedRange = previousActiveRange.difference(activeRange);
			const checkPolarity = state.checked.has(state.rangeAnchor);

			return {
				checked: state.checked
					.union(checkPolarity ? activeRange : deactivatedRange)
					.difference(checkPolarity ? deactivatedRange : activeRange),
				rangeAnchor: state.rangeAnchor,
				rangeEnd: evt.item,
			};
		}

		const checked = new Set(state.checked);
		if (checked.has(evt.item)) checked.delete(evt.item);
		else checked.add(evt.item);

		return {
			checked,
			rangeAnchor: evt.item,
			rangeEnd: evt.item,
		};
	};
