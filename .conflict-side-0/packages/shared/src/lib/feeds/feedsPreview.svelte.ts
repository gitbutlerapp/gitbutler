import { feedsSelectors } from '$lib/feeds/feedsSlice';
import { postsSelectors } from '$lib/feeds/postsSlice';
import {
	registerInterest,
	registerInterestInView
} from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import { userByLoginTable, userTable } from '$lib/users/usersSlice';
import type { FeedService } from '$lib/feeds/service';
import type { Feed, Post } from '$lib/feeds/types';
import type { Loadable } from '$lib/network/types';
import type { AppFeedsState, AppPostsState, AppUsersState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';
import type { User } from '$lib/users/types';
import type { UserService } from '$lib/users/userService';

export function getFeed(
	appState: AppFeedsState,
	feedService: FeedService,
	identity?: string
): Reactive<Feed | undefined> {
	// Fetching the head of the feed
	$effect(() => {
		if (!identity) return;

		const interest = feedService.getFeedHeadInterest(identity);
		registerInterest(interest);
	});

	// List posts associated with the feed
	const feed = $derived(identity ? feedsSelectors.selectById(appState.feeds, identity) : undefined);

	return {
		get current() {
			return feed;
		}
	};
}

export function getFeedLastPost(
	appState: AppFeedsState & AppPostsState,
	feedService: FeedService,
	feed?: Feed
): Reactive<Post | undefined> {
	const lastPostId = $derived(feed?.postIds.at(-1));
	$effect(() => {
		if (!lastPostId) return;

		const postWithRepliesInterest = feedService.getPostWithRepliesInterest(lastPostId);
		registerInterest(postWithRepliesInterest);
	});
	const lastPost = $derived(
		lastPostId ? postsSelectors.selectById(appState.posts, lastPostId) : undefined
	);

	return {
		get current() {
			return lastPost;
		}
	};
}

export function getPostAuthor(
	appState: AppPostsState & AppUsersState,
	feedService: FeedService,
	userService: UserService,
	postId: string,
	renderInView?: {
		element?: HTMLElement;
	}
): Reactive<Loadable<User> | undefined> {
	const currentUserId = $derived.by(() => {
		const postInterest = feedService.getPostWithRepliesInterest(postId);
		if (renderInView) {
			registerInterestInView(postInterest, renderInView.element);
		} else {
			registerInterest(postInterest);
		}
		const post = postsSelectors.selectById(appState.posts, postId);

		if (!post) return;

		const userByLoginInterest = userService.getUserByLoginInterest(post.userLogin);
		if (renderInView) {
			registerInterestInView(userByLoginInterest, renderInView.element);
		} else {
			registerInterest(userByLoginInterest);
		}

		return userByLoginTable.selectors.selectById(appState.usersByLogin, post.userLogin);
	});

	const current = $derived.by(() => {
		if (!currentUserId) return undefined;
		if (!isFound(currentUserId)) return currentUserId;
		const id = currentUserId.value;
		return userTable.selectors.selectById(appState.users, id);
	});

	return {
		get current() {
			return current;
		}
	};
}
