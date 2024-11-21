<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/redux/interest/RegisterInterest.svelte';
	import { FeedService } from '@gitbutler/shared/redux/posts/service';
	import { postSelectors, postRepliesSelectors } from '@gitbutler/shared/redux/posts/slice';
	import { AppState } from '@gitbutler/shared/redux/store';

	type Props = {
		postId: string;
	};

	const { postId }: Props = $props();

	const feedService = getContext(FeedService);

	const appState = getContext(AppState);
	const postState = appState.post;
	const postRepliesState = appState.postReplies;

	const postWithRepliesInterest = $derived(feedService.getPostWithRepliesInterest(postId));
	const post = $derived(postSelectors.selectById($postState, postId));
	const postReplies = $derived(postRepliesSelectors.selectById($postRepliesState, postId));

	let postCardRef = $state<HTMLDivElement | undefined>(undefined);

	$effect(() => console.log(post));
	$effect(() => console.log(postReplies));
</script>

<RegisterInterest interest={postWithRepliesInterest} reference={postCardRef} onlyInView />

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
