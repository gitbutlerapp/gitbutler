<script lang="ts">
	import Post from '$lib/feeds/Post.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { feedsSelectors } from '@gitbutler/shared/feeds/feedsSlice';
	import { postsSelectors } from '@gitbutler/shared/feeds/postsSlice';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';

	const appState = getContext(AppState);
	const feedService = getContext(FeedService);

	// Fetching the head of the feed
	const feedHeadInterest = feedService.getFeedHeadInterest();
	// List posts associated with the feed
	const feed = $derived(feedsSelectors.selectById(appState.feeds, 'all'));

	// Post creation
	let newPostContent = $state('');
	function createPost() {
		feedService.createPost(newPostContent);
		newPostContent = '';
	}

	// Infinite scrolling
	const lastPostId = $derived(feed?.postIds.at(-1));
	const lastPostInterest = $derived(
		lastPostId ? feedService.getPostWithRepliesInterest(lastPostId) : undefined
	);
	const lastPost = $derived(
		lastPostId ? postsSelectors.selectById(appState.posts, lastPostId) : undefined
	);
	let lastElement = $state<HTMLElement | undefined>();

	$effect(() => {
		if (lastElement) {
			const observer = new IntersectionObserver(
				(entries) => {
					if (entries[0]?.isIntersecting && lastPost) {
						feedService.getFeedPage('all', lastPost?.createdAt);
					}
				},
				{ root: null }
			);

			observer.observe(lastElement);
			return () => observer.disconnect();
		}
	});
</script>

<RegisterInterest interest={feedHeadInterest} />

{#if lastPostInterest}
	<RegisterInterest interest={lastPostInterest} />
{/if}

<ScrollableContainer>
	<div>
		<input type="text" bind:value={newPostContent} />
		<Button onclick={createPost}>Create</Button>
	</div>

	<div>
		{#if feed}
			{#each feed.postIds as postId, index (postId)}
				{#if index < feed.postIds.length - 1 && lastPostInterest}
					<div bind:this={lastElement}></div>
				{/if}

				<Post {postId} />
			{/each}
		{/if}
	</div>
</ScrollableContainer>
