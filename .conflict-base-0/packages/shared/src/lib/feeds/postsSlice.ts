import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { Post } from '$lib/feeds/types';

const postsAdapter = createEntityAdapter({
	selectId: (post: Post) => post.uuid,
	sortComparer: (a: Post, b: Post) => a.content.localeCompare(b.content)
});

const postsSlice = createSlice({
	name: 'posts',
	initialState: postsAdapter.getInitialState(),
	reducers: {
		addPost: postsAdapter.addOne,
		addPosts: postsAdapter.addMany,
		removePost: postsAdapter.removeOne,
		removePosts: postsAdapter.removeMany,
		upsertPost: postsAdapter.upsertOne,
		upsertPosts: postsAdapter.upsertMany
	}
});

export const postsReducer = postsSlice.reducer;

export const postsSelectors = postsAdapter.getSelectors();
export const { addPost, addPosts, removePost, removePosts, upsertPost, upsertPosts } =
	postsSlice.actions;
