import { syncQueryParams } from '$lib/syncQueryParams.svelte';
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

export class ReviewSectionsService {
	paramsIfKeyMatches: (
		key: string
	) => { selectedBefore: number | undefined; selectedAfter: number | undefined } | undefined;

	constructor(
		private readonly webState: WebState,
		private readonly appDispatch: AppDispatch,
		routes: WebRoutesService
	) {
		const { paramsIfKeyMatches } = syncQueryParams({
			getUrlKey: () => {
				const changeId = $derived(routes.isProjectReviewBranchCommitPageSubset?.changeId);
				return reactive(() => changeId);
			},
			getRecord: (changeId) => {
				const section = $derived(
					reviewSectionSelectors.selectById(this.webState.reviewSections, changeId)
				);
				return reactive(() => section);
			},
			parseParams: (query) => {
				const rawSelectedAfter = query.get('selectedAfter');
				const selectedAfter = rawSelectedAfter ? parseInt(rawSelectedAfter) : undefined;
				const rawSelectedBefore = query.get('selectedBefore');
				const selectedBefore = rawSelectedBefore ? parseInt(rawSelectedBefore) : undefined;

				return { selectedAfter, selectedBefore };
			},
			// The query params have changed (via user navigation), update the
			// record
			updateRecord: (record, { selectedBefore, selectedAfter }) => {
				this.setSelection(record.changeId, { selectedBefore, selectedAfter });
			},
			// The record has changed, update the query params
			updateParams: (record, query) => {
				const { selectedAfter, selectedBefore, lastKnownVersion } = record;
				if (selectedAfter === lastKnownVersion) {
					query.delete('selectedAfter');
				} else {
					query.set('selectedAfter', String(selectedAfter));
				}

				if (selectedBefore === beforeIndex) {
					query.delete('selectedBefore');
				} else {
					query.set('selectedBefore', String(selectedBefore));
				}
			}
		});
		this.paramsIfKeyMatches = paramsIfKeyMatches;

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
				selectedBefore: beforeIndex,
				selectedAfter: patchCommit.version
			};
		} else {
			// If there is a relevant set of query params for the current target
			// we should read from the query params.
			const params = this.paramsIfKeyMatches(patchCommit.changeId);

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
