import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { Section } from '$lib/branchReviews/types';

const reviewSectionsAdapter = createEntityAdapter({
	selectId: (reviewSection: Section) => reviewSection.id
});

const reviewSectionsSlice = createSlice({
	name: 'reviewSections',
	initialState: reviewSectionsAdapter.getInitialState(),
	reducers: {
		addReviewSection: reviewSectionsAdapter.addOne,
		addReviewSections: reviewSectionsAdapter.addMany,
		removeReviewSection: reviewSectionsAdapter.removeOne,
		removeReviewSections: reviewSectionsAdapter.removeMany,
		upsertReviewSection: reviewSectionsAdapter.upsertOne,
		upsertReviewSections: reviewSectionsAdapter.upsertMany
	}
});

export const reviewSectionsReducer = reviewSectionsSlice.reducer;

export const reviewSectionsSelectors = reviewSectionsAdapter.getSelectors();
export const {
	addReviewSection,
	addReviewSections,
	removeReviewSection,
	removeReviewSections,
	upsertReviewSection,
	upsertReviewSections
} = reviewSectionsSlice.actions;
