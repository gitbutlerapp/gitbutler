<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import { postsSelectors } from '@gitbutler/shared/feeds/postsSlice';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import { AppState } from '@gitbutler/shared/redux/store';

	type Props = {
		postId: string;
	};

	const { postId }: Props = $props();

	const feedService = getContext(FeedService);

	const appState = getContext(AppState);
	const postsState = appState.posts;

	const postWithRepliesInterest = $derived(feedService.getPostWithRepliesInterest(postId));
	const post = $derived(postsSelectors.selectById($postsState, postId));

	let postCardRef = $state<HTMLDivElement | undefined>(undefined);

	$effect(() => console.log(post));
</script>

<RegisterInterest interest={postWithRepliesInterest} reference={postCardRef} onlyInView />

{#if post}
	<div class="card card__content" bind:this={postCardRef}>
		<p>{post.uuid}</p>
		<p>{post.content}</p>
		{#if post.replyIds}
			<p>There is {post.replyIds.length} replies</p>
		{:else}
			<p>Loading replies count...</p>
		{/if}
	</div>
{:else}
	<p>Loading...</p>
{/if}
