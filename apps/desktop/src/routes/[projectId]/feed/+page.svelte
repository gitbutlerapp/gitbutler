<script lang="ts">
	import Post from '$lib/posts/Post.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Interest from '@gitbutler/shared/redux/interest/Interest.svelte';
	import { FeedService } from '@gitbutler/shared/redux/posts/service';
	import { feedSelectors } from '@gitbutler/shared/redux/posts/slice';
	import { useStore } from '@gitbutler/shared/redux/utils';
	import Button from '@gitbutler/ui/Button.svelte';

	const store = useStore();
	const feedService = getContext(FeedService);

	const feedInterest = feedService.getFeedInterest();
	const feed = $derived(feedSelectors.selectById($store, 'all'));

	$effect(() => console.log(feed));

	let newPostContent = $state('');
	function createPost() {
		feedService.createPost(newPostContent);
		newPostContent = '';
	}
</script>

<Interest interest={feedInterest} />
<ScrollableContainer>
	<div>
		<input type="text" bind:value={newPostContent} />
		<Button onclick={createPost}>Create</Button>
	</div>

	<div>
		{#if feed}
			{#each feed.postIds as postId (postId)}
				<Post {postId} />
			{/each}
		{/if}
	</div>
</ScrollableContainer>
