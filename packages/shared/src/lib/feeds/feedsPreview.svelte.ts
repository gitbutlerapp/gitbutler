import { feedsSelectors } from '$lib/feeds/feedsSlice';
import { postsSelectors } from '$lib/feeds/postsSlice';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { usersSelectors } from '$lib/users/usersSlice';
import type { FeedService } from '$lib/feeds/service';
import type { Feed, Post } from '$lib/feeds/types';
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

export function getPost(
	appState: AppPostsState,
	feedService: FeedService,
	postId: string,
	inView?: InView
): Reactive<Post | undefined> {
	$effect(() => {
		const interest = feedService.getPostWithRepliesInterest(postId);
		registerInterest(interest, inView);
	});
	const post = $derived(postsSelectors.selectById(appState.posts, postId));

	return {
		get current() {
			return post;
		}
	};
}

export function getPostAuthor(
	appState: AppPostsState & AppUsersState,
	feedService: FeedService,
	userService: UserService,
	postId: string,
	inView?: InView
): Reactive<User | undefined> {
	const post = getPost(appState, feedService, postId, inView);

	const current = $derived.by(() => {
		if (!post.current) return;

		const userInterest = userService.getUserInterest(post.current.userLogin);
		registerInterest(userInterest, inView);

		return usersSelectors.selectById(appState.users, post.current.userLogin);
	});

	return {
		get current() {
			return current;
		}
	};
}
