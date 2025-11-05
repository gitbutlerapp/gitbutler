import { createEntityAdapter, createSlice, type PayloadAction } from '@reduxjs/toolkit';
import type { Feed } from '$lib/feeds/types';

// Feeds
const feedsAdapter = createEntityAdapter({
	selectId: (feed: Feed) => feed.identifier,
	sortComparer: (a: Feed, b: Feed) => a.identifier.localeCompare(b.identifier)
});

const feedsSlice = createSlice({
	name: 'feeds',
	initialState: feedsAdapter.getInitialState(),
	reducers: {
		addFeed: feedsAdapter.addOne,
		removeFeed: feedsAdapter.removeOne,
		feedAppend: (state, action: PayloadAction<{ identifier: string; postIds: string[] }>) => {
			let feed = state.entities[action.payload.identifier];
			if (!feed) feed = { identifier: action.payload.identifier, postIds: [] };

			const postIdsToAdd = action.payload.postIds.filter(
				(postId) => !feed.postIds.includes(postId)
			);
			feed.postIds.push(...postIdsToAdd);

			feedsAdapter.upsertOne(state, feed);
		},
		feedPrepend: (state, action: PayloadAction<{ identifier: string; postIds: string[] }>) => {
			let feed = state.entities[action.payload.identifier];
			if (!feed) feed = { identifier: action.payload.identifier, postIds: [] };

			const postIdsToAdd = action.payload.postIds.filter(
				(postId) => !feed.postIds.includes(postId)
			);
			feed.postIds.unshift(...postIdsToAdd);

			feedsAdapter.upsertOne(state, feed);
		}
	}
});

export const feedsReducer = feedsSlice.reducer;

export const feedsSelectors = feedsAdapter.getSelectors();
export const { addFeed, removeFeed, feedAppend, feedPrepend } = feedsSlice.actions;
