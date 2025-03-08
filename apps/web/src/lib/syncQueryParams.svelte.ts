import { shallowCompare } from '@gitbutler/shared/shallowCompare';
import { untrack } from 'svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import { replaceState } from '$app/navigation';

type SyncedQueryParams<ParsedParams, Record, Key> = {
	/**
	 * A reactive method that returns the key of the current subject of the
	 * page.
	 *
	 * Typically, this would be used to determine "am I on the commit page"
	 * and if that is true, return the identifier of the commit, like the
	 * `changeId`.
	 *
	 * Most pages have some form of subject, as they are displaying information
	 * about one main data strucutre, whether it be a project, branch,
	 * or commit. If you happen to be on the root path, or a constant path like
	 * `/search`, then return a constant string when the route is active.
	 *
	 * An example implementation would be:
	 * ```ts
	 *	getUrlKey: () => {
	 *		// `routes` is injected so it is in scope of the call to
	 *		// `hasSyncedQueryParams`.
	 *		const changeId = $derived(routes.isProjectReviewBranchCommitPageSubset?.changeId);
	 *		return reactive(() => changeId);
	 *	},
	 * ```
	 */
	getUrlKey: () => Reactive<Key | undefined>;
	/**
	 * A reactive method that takes the key defined in `getUrlKey` and fetches
	 * the cooresponding record. The key is always present when this method is
	 * called.
	 *
	 * An example implementation would be:
	 * ```ts
	 */
	getRecord: (key: Key) => Reactive<Record | undefined>;
	/**
	 * A non-reactive method used to parse the relevant parts of the query
	 * string.
	 *
	 * The result of this function gets passed to `updateRecord` and is
	 * returned from `paramsIfKeyMatches`
	 */
	parseParams: (query: URLSearchParams) => ParsedParams;
	/**
	 * A non-reactive method that gets called whenever the redux record should
	 * be updated.
	 *
	 * The method is provided with the current record value the parsed params.
	 *
	 * The method should dispatch to redux on it's own.
	 *
	 * This method is called after navigation when there is a record returned
	 * from getRecord.
	 */
	updateRecord: (record: Record, params: ParsedParams) => void;
	/**
	 * A non-reactive method that gets called whenever the query params should
	 * be updated.
	 *
	 * This method MUST be deterministic.
	 *
	 * The method is provided with the current record value the parsed params.
	 *
	 * The method should mutate the query property.
	 *
	 * This method is called after `getRecord` emits a new value.
	 */
	updateParams: (record: Record, query: URLSearchParams) => void;
};
type SyncedQueryParamsReturn<ParseParams, Key> = {
	/**
	 * A non-reactive method for getting the current paramaters.
	 *
	 * This requires you to pass the key of the record you want to fetch.
	 *
	 * This is to ensure you're not trying to apply paramaters that belong to
	 * the wrong record.
	 */
	paramsIfKeyMatches: (key: Key) => ParseParams | undefined;
};

/**
 * A function that helps manage syncing a redux (or other) store with the query
 * paramaters.
 *
 * The goal of this is to allow you to use redux as a source of truth, which
 * writes to the query paramaters whenever redux changes, and only reads from
 * the query params after a navigation.
 *
 * A lifecycle looks like:
 *
 * - First visit page & params exist
 *   - When the redux record is about to be created, we use the values from the
 *     the query params via `paramsIfKeyMatches` when the coresponding redux
 *     redux record is made.
 * - The redux record gets updated.
 *   - We call `updateParams` with the updated redux record, and update the URL
 *     to include the new query params if they have changed.
 * - Navigate to another page with query params
 *   - If there is already a redux record when we navigate with query params,
 *     we call `updateRecord` with the query params and current redux record.
 * - Navigate back to first page without params
 *   - Since there is a redux record that exists, we call `updateParams` and
 *     update the URL again to include the values from the redux record.
 */
export function syncQueryParams<ParsedParams, Record, Key>(
	params: SyncedQueryParams<ParsedParams, Record, Key>
): SyncedQueryParamsReturn<ParsedParams, Key> {
	// NOTE: In the following implementation, I've been careful to use
	// `untrack` when calling user defined functions, even though they are
	// descrived as non-reactive. This is to prevent any accidental reactivity
	// intefearing with the lifecycle.

	let oldKey: Key;
	let oldRecord: Record;

	// This effect depends on the specified key and record.
	// This means that this re-runs whenever we navigate to a different page
	// and the key changes and whenever the result of `getRecord` changes.
	$effect(() => {
		const key = params.getUrlKey().current;
		if (!key) return;
		const value = params.getRecord(key).current;
		// Returning early when there is no value allows some other part of the
		// code to make use of `paramsIfKeyMatches` to read from any potential
		// query params when constructing what should be this value.
		if (!value) return;

		// If the value hasn't changed, we have not navigated and there
		// is nothing interesting to update the query params with.
		if (shallowCompare(value, oldRecord)) return;
		oldRecord = value;

		const query = new URLSearchParams(location.search);
		const originalQueryString = query.toString();

		if (oldKey !== key) {
			// If we have navigated and we have a record, we want to update the
			// record to match the value in the query params.
			const parsedQuery = untrack(() => params.parseParams(query));
			untrack(() => params.updateRecord(value, parsedQuery));
		}
		oldKey = key;

		// Even if we have navigated, we always want to call update params,
		// even if there were params provided when we first navigated. This is
		// to make sure the query params always get restored.
		// The updateRecord call may have made the `value` we have here
		// obsolete, but we have no good way of knowing that. (it is also a
		// very rare condition, as it would require a `goto()` call with query
		// params provided.
		untrack(() => params.updateParams(value, query));

		const newQueryString = query.toString();
		if (newQueryString !== originalQueryString) {
			replaceState(`?${newQueryString}`, {});
		}
	});

	return {
		paramsIfKeyMatches: (key) => {
			const currentKey = untrack(() => params.getUrlKey().current);
			if (currentKey === key) {
				const query = new URLSearchParams(location.search);
				return params.parseParams(query);
			}
		}
	};
}
