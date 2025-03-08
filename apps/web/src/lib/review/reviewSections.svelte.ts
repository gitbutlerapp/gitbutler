import { isFound } from '@gitbutler/shared/network/loadable';
import { patchCommitsSelector } from '@gitbutler/shared/patches/patchCommitsSlice';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import { untrack } from 'svelte';
import type { WebState } from '$lib/redux/store.svelte';
import type { PatchCommit } from '@gitbutler/shared/patches/types';
import type { AppDispatch } from '@gitbutler/shared/redux/store.svelte';
import type { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import { afterNavigate, replaceState } from '$app/navigation';

const beforeIndex = -1;

type ReviewSectionData = {
	changeId: string;
	lastKnownVersion: number;
	selectedBefore: number;
	selectedAfter: number;
};

const reviewSectionsAdapter = createEntityAdapter<ReviewSectionData, ReviewSectionData['changeId']>(
	{
		selectId: (reviewSection: ReviewSectionData) => reviewSection.changeId
	}
);

const reviewSectionsSlice = createSlice({
	name: 'reviewSelections',
	initialState: reviewSectionsAdapter.getInitialState(),
	reducers: {
		updateReviewSection: reviewSectionsAdapter.updateOne,
		upsertReviewSections: reviewSectionsAdapter.upsertMany
	}
});

export const reviewSectionsReducer = reviewSectionsSlice.reducer;

export const reviewSectionSelectors = reviewSectionsAdapter.getSelectors();
const { updateReviewSection, upsertReviewSections } = reviewSectionsSlice.actions;

type QueryParamsData = {
	selectedBefore?: number;
	selectedAfter?: number;
};

/**
 * Helps manage the query string for the review selections.
 *
 * This avoids storing the beforeIndex or the latest version index.
 */
class QueryParams {
	constructor(private readonly routes: WebRoutesService) {}

	get(changeId: string): QueryParamsData | undefined {
		if (!untrack(() => this.onRelevantRouteFor(changeId).current)) return;

		const url = new URL(location.toString());
		const rawSelectedAfter = url.searchParams.get('selectedAfter');
		const rawSelectedBefore = url.searchParams.get('selectedBefore');

		return {
			selectedAfter: rawSelectedAfter ? parseInt(rawSelectedAfter) : undefined,
			selectedBefore: rawSelectedBefore ? parseInt(rawSelectedBefore) : undefined
		};
	}

	set(changeId: string, { selectedBefore, selectedAfter, lastKnownVersion }: ReviewSectionData) {
		if (!untrack(() => this.onRelevantRouteFor(changeId).current)) return;

		const url = new URL(location.toString());
		const searchParams = url.searchParams;
		const originalQuery = searchParams.toString();

		if (isDefined(selectedAfter)) {
			if (selectedAfter === lastKnownVersion) {
				searchParams.delete('selectedAfter');
			} else {
				searchParams.set('selectedAfter', String(selectedAfter));
			}
		}

		if (isDefined(selectedBefore)) {
			if (selectedBefore === -1) {
				searchParams.delete('selectedBefore');
			} else {
				searchParams.set('selectedBefore', String(selectedBefore));
			}
		}

		const newQuery = searchParams.toString();
		if (newQuery === originalQuery) return;
		replaceState(`?${searchParams.toString()}`, {});
	}

	private onRelevantRouteFor(changeId: string) {
		const onRelevant = $derived(this.relevantTarget.current === changeId);
		return reactive(() => onRelevant);
	}

	/**
	 * Are we on a page that might have a relevant redux entry
	 *
	 * Returns a changeId
	 */
	get relevantTarget(): Reactive<string | undefined> {
		const changeId = $derived.by(() => {
			const path = this.routes.isProjectReviewBranchCommitPageSubset;
			if (!isDefined(path)) return;
			return path.changeId;
		});
		return reactive(() => changeId);
	}
}

export class ReviewSectionsService {
	private readonly queryParams: QueryParams;

	constructor(
		private readonly webState: WebState,
		private readonly appDispatch: AppDispatch,
		routes: WebRoutesService
	) {
		this.queryParams = new QueryParams(routes);

		// After navigation, we should look to see if there we are on a page
		// where the cooresponding slice might have relevant data for the
		// query string.
		afterNavigate(() => {
			const changeId = untrack(() => this.queryParams.relevantTarget.current);
			if (!changeId) return;

			const target = untrack(() =>
				reviewSectionSelectors.selectById(this.webState.reviewSections, changeId)
			);
			if (!target) return;

			this.setSelection(changeId, target);
		});

		// After a relevant piece of state has been set, we should also update
		// the query params.
		const currentPageChangeId = $derived(this.queryParams.relevantTarget.current);
		const currentPageSelection = $derived(
			currentPageChangeId
				? reviewSectionSelectors.selectById(this.webState.reviewSections, currentPageChangeId)
				: undefined
		);
		$effect(() => {
			if (!currentPageChangeId || !currentPageSelection) return;

			this.queryParams.set(currentPageChangeId, currentPageSelection);
		});

		// Update selections based on latest patch commit informtion
		const patchCommits = $derived(patchCommitsSelector.selectAll(webState.patches));
		$effect(() => {
			const updates: ReviewSectionData[] = [];
			patchCommits.forEach((patchCommit) => {
				if (!isFound(patchCommit)) return;
				const update = this.handlePatchUpdate(patchCommit.value);
				if (update) {
					updates.push(update);
				}
			});

			this.appDispatch.dispatch(upsertReviewSections(updates));
		});
	}

	private handlePatchUpdate(patchCommit: PatchCommit) {
		if (!patchCommit.version) return;
		const reviewSection = reviewSectionSelectors.selectById(
			untrack(() => this.webState.reviewSections),
			patchCommit.changeId
		);

		// If the review section version matches what we already know, then
		// don't do anything.
		if (reviewSection?.lastKnownVersion === patchCommit.version) return;

		// If there is an existing review section, update it to have
		let updatedReviewSection: ReviewSectionData;
		if (reviewSection) {
			// If we have an existing review section, we do want to update it
			updatedReviewSection = {
				...reviewSection,
				lastKnownVersion: patchCommit.version,
				selectedBefore: -1,
				selectedAfter: patchCommit.version
			};
		} else {
			// If there is a relevant set of query params for the current target
			// we should read from the query params.
			const params = this.queryParams.get(patchCommit.changeId);
			let selectedAfter;
			if (isDefined(params?.selectedAfter) && params.selectedAfter <= patchCommit.version) {
				selectedAfter = params.selectedAfter;
			} else {
				selectedAfter = patchCommit.version;
			}

			updatedReviewSection = {
				changeId: patchCommit.changeId,
				lastKnownVersion: patchCommit.version,
				selectedBefore: params?.selectedBefore ?? beforeIndex,
				selectedAfter
			};
		}
		return updatedReviewSection;
	}

	allOptions(changeId: string): Reactive<[number, string][]> {
		const reviewSection = $derived(
			reviewSectionSelectors.selectById(this.webState.reviewSections, changeId)
		);

		const options = $derived.by(() => {
			if (!reviewSection) return [];

			const out: [number, string][] = [[beforeIndex, 'Base']];
			for (let i = 0; i !== reviewSection.lastKnownVersion; ++i) {
				out.push([i + 1, `v${i + 1}`]);
			}
			return out;
		});

		return reactive(() => options);
	}

	currentSelection(
		changeId: string
	): Reactive<{ selectedBefore: number; selectedAfter: number } | undefined> {
		const reviewSection = $derived(
			reviewSectionSelectors.selectById(this.webState.reviewSections, changeId)
		);
		return reactive(() => reviewSection);
	}

	setSelection(changeId: string, params: { selectedBefore?: number; selectedAfter?: number }) {
		const changes: Partial<ReviewSectionData> = {};
		if (params.selectedAfter) changes.selectedAfter = params.selectedAfter;
		if (params.selectedBefore) changes.selectedBefore = params.selectedBefore;
		this.appDispatch.dispatch(
			updateReviewSection({
				id: changeId,
				changes
			})
		);
	}
}
