<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Interest from '@gitbutler/shared/redux/interest/Interest.svelte';
	import { FeedService } from '@gitbutler/shared/redux/posts/service';
	import { postSelectors, postRepliesSelectors } from '@gitbutler/shared/redux/posts/slice';
	import { useStore } from '@gitbutler/shared/redux/utils';

	type Props = {
		postId: string;
	};

	const { postId }: Props = $props();

	const feedService = getContext(FeedService);

	const store = useStore();
	const postWithRepliesInterest = $derived(feedService.getPostWithRepliesInterest(postId));
	const post = $derived(postSelectors.selectById($store, postId));
	const postReplies = $derived(postRepliesSelectors.selectById($store, postId));

	let postCardRef = $state<HTMLDivElement | undefined>(undefined);

	$effect(() => console.log(post));
	$effect(() => console.log(postReplies));
</script>

<Interest interest={postWithRepliesInterest} reference={postCardRef} onlyInView />

{#if post}
	<div class="card card__content" bind:this={postCardRef}>
		<p>{post.uuid}</p>
		<p>{post.content}</p>
		{#if postReplies}
			<p>There is {postReplies.replyIds.length} replies</p>
		{:else}
			<p>Loading replies count...</p>
		{/if}
	</div>
{:else}
	<p>Loading...</p>
{/if}
