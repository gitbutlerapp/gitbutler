import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { Feed, Post, PostReplies } from '$lib/redux/posts/types';
import type { RootState } from '$lib/redux/store';

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

export const postSelectors = postAdapter.getSelectors((state: RootState) => state.post);
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
		addFeedPost: (state, action) => {
			const feed = state.entities[action.payload.feedId];
			if (feed) {
				feed.postIds.unshift(action.payload.postId);
			}
		}
	}
});

export const feedReducer = feedSlice.reducer;

export const feedSelectors = feedAdapter.getSelectors((state: RootState) => state.feed);
export const { addFeed, updateFeed, removeFeed, upsertFeed } = feedSlice.actions;

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

export const postRepliesSelectors = postRepliesAdapter.getSelectors(
	(state: RootState) => state.postReplies
);
export const { addPostReplies, updatePostReplies, removePostReplies, upsertPostReplies } =
	postRepliesSlice.actions;
