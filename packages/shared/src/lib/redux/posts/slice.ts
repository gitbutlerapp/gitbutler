import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { Post } from '$lib/redux/posts/types';
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
