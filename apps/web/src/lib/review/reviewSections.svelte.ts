import { isFound } from '@gitbutler/shared/network/loadable';
import { patchCommitsSelector } from '@gitbutler/shared/patches/patchCommitsSlice';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import { untrack } from 'svelte';
import type { WebState } from '$lib/redux/store.svelte';
import type { PatchCommit } from '@gitbutler/shared/patches/types';
import type { AppDispatch } from '@gitbutler/shared/redux/store.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

function reviewSectionKey(branchUuid: string, changeId: string): string {
	return `${branchUuid}/${changeId}`;
}

type ReviewSectionData = {
	key: string;
	lastKnownVersion: number;
	selectedBefore: number;
	selectedAfter: number;
};

const reviewSectionsAdapter = createEntityAdapter<ReviewSectionData, ReviewSectionData['key']>({
	selectId: (reviewSection: ReviewSectionData) => reviewSection.key
});

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
	constructor(
		private readonly webState: WebState,
		private readonly appDispatch: AppDispatch
	) {
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
		const key = reviewSectionKey(patchCommit.branchUuid, patchCommit.changeId);
		const reviewSection = reviewSectionSelectors.selectById(
			untrack(() => this.webState.reviewSections),
			key
		);

		// If the review section version matches what we already know, then
		// don't do anything.
		if (reviewSection?.lastKnownVersion === patchCommit.version) return;

		// If there is an existing review section, update it to have
		let updatedReviewSection: ReviewSectionData;
		if (reviewSection) {
			updatedReviewSection = {
				...reviewSection,
				lastKnownVersion: patchCommit.version,
				selectedBefore: -1,
				selectedAfter: patchCommit.version
			};
		} else {
			updatedReviewSection = {
				key,
				lastKnownVersion: patchCommit.version,
				selectedBefore: -1,
				selectedAfter: patchCommit.version
			};
		}
		return updatedReviewSection;
	}

	allOptions(branchUuid: string, changeId: string): Reactive<[number, string][]> {
		const key = reviewSectionKey(branchUuid, changeId);
		const reviewSection = $derived(
			reviewSectionSelectors.selectById(this.webState.reviewSections, key)
		);

		const options = $derived.by(() => {
			if (!reviewSection) return [];

			const out: [number, string][] = [[-1, 'Base']];
			for (let i = 0; i !== reviewSection.lastKnownVersion; ++i) {
				out.push([i + 1, `v${i + 1}`]);
			}
			return out;
		});

		return reactive(() => options);
	}

	currentSelection(
		branchUuid: string,
		changeId: string
	): Reactive<{ selectedBefore: number; selectedAfter: number } | undefined> {
		const key = reviewSectionKey(branchUuid, changeId);
		const reviewSection = $derived(
			reviewSectionSelectors.selectById(this.webState.reviewSections, key)
		);
		return reactive(() => reviewSection);
	}

	setSelection(
		branchUuid: string,
		changeId: string,
		params: { selectedBefore?: number; selectedAfter?: number }
	) {
		const key = reviewSectionKey(branchUuid, changeId);
		const changes: Partial<ReviewSectionData> = {};
		if (params.selectedAfter) changes.selectedAfter = params.selectedAfter;
		if (params.selectedBefore) changes.selectedBefore = params.selectedBefore;
		this.appDispatch.dispatch(
			updateReviewSection({
				id: key,
				changes
			})
		);
	}
}
