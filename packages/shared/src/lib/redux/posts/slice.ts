import { createEntityAdapter, createSlice, type PayloadAction } from '@reduxjs/toolkit';
import type { Feed, Post, PostReplies } from '$lib/redux/posts/types';

const postAdapter = createEntityAdapter({
	selectId: (post: Post) => post.uuid,
	sortComparer: (a: Post, b: Post) => a.content.localeCompare(b.content)
});

const postSlice = createSlice({
	name: 'posts',
	initialState: postAdapter.getInitialState(),
	reducers: {
		addPost: postAdapter.addOne,
		addPosts: postAdapter.addMany,
		updatePost: postAdapter.updateOne,
		updatePosts: postAdapter.updateMany,
		removePost: postAdapter.removeOne,
		removePosts: postAdapter.removeMany,
		upsertPost: postAdapter.upsertOne,
		upsertPosts: postAdapter.upsertMany
	}
});

export const postReducer = postSlice.reducer;

export const postSelectors = postAdapter.getSelectors();
export const {
	addPost,
	addPosts,
	updatePost,
	updatePosts,
	removePost,
	removePosts,
	upsertPost,
	upsertPosts
} = postSlice.actions;

// Feeds
const feedAdapter = createEntityAdapter({
	selectId: (feed: Feed) => feed.identifier,
	sortComparer: (a: Feed, b: Feed) => a.identifier.localeCompare(b.identifier)
});

const feedSlice = createSlice({
	name: 'feeds',
	initialState: feedAdapter.getInitialState(),
	reducers: {
		addFeed: feedAdapter.addOne,
		updateFeed: feedAdapter.updateOne,
		removeFeed: feedAdapter.removeOne,
		upsertFeed: feedAdapter.upsertOne,
		feedAppend: (state, action: PayloadAction<{ identifier: string; postIds: string[] }>) => {
			let feed = state.entities[action.payload.identifier];
			if (!feed) feed = { identifier: action.payload.identifier, postIds: [] };

			const postIdsToAdd = action.payload.postIds.filter(
				(postId) => !feed.postIds.includes(postId)
			);
			feed.postIds.push(...postIdsToAdd);

			feedAdapter.upsertOne(state, feed);
		},
		feedPrepend: (state, action: PayloadAction<{ identifier: string; postIds: string[] }>) => {
			let feed = state.entities[action.payload.identifier];
			if (!feed) feed = { identifier: action.payload.identifier, postIds: [] };

			const postIdsToAdd = action.payload.postIds.filter(
				(postId) => !feed.postIds.includes(postId)
			);
			feed.postIds.unshift(...postIdsToAdd);

			feedAdapter.upsertOne(state, feed);
		}
	}
});

export const feedReducer = feedSlice.reducer;

export const feedSelectors = feedAdapter.getSelectors();
export const { addFeed, updateFeed, removeFeed, upsertFeed, feedAppend, feedPrepend } =
	feedSlice.actions;

// Replies
const postRepliesAdapter = createEntityAdapter({
	selectId: (postReplies: PostReplies) => postReplies.postId,
	sortComparer: (a: PostReplies, b: PostReplies) => a.postId.localeCompare(b.postId)
});

const postRepliesSlice = createSlice({
	name: 'postReplies',
	initialState: postRepliesAdapter.getInitialState(),
	reducers: {
		addPostReplies: postRepliesAdapter.addOne,
		updatePostReplies: postRepliesAdapter.updateOne,
		removePostReplies: postRepliesAdapter.removeOne,
		upsertPostReplies: postRepliesAdapter.upsertOne,
		addReply: (state, action) => {
			const feed = state.entities[action.payload.postId];
			if (feed) {
				feed.replyIds.unshift(action.payload.postId);
			}
		}
	}
});

export const postRepliesReducer = postRepliesSlice.reducer;

export const postRepliesSelectors = postRepliesAdapter.getSelectors();
export const { addPostReplies, updatePostReplies, removePostReplies, upsertPostReplies } =
	postRepliesSlice.actions;
