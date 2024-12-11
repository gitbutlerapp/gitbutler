<script lang="ts">
	import CreatePost from '$lib/feeds/CreatePost.svelte';
	import Post from '$lib/feeds/Post.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { getPost } from '@gitbutler/shared/feeds/feedsPreview.svelte';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { page } from '$app/stores';

	const feedService = getContext(FeedService);
	const appState = getContext(AppState);

	const postId = $derived($page.params.postId);

	const post = $derived.by(() => {
		if (!postId) return;

		return getPost(appState, feedService, postId);
	});
</script>

<div class="page">
	<div class="page-content">
		{#if post?.current}
			<ScrollableContainer wide>
				<div class="bleep-container">
					<Button
						onclick={() => {
							history.back();
						}}>Back</Button
					>
				</div>

				<div class="bleep-container">
					<Post postId={post.current.uuid} />
				</div>

				<hr />

				<div class="bleep-container">
					<CreatePost replyTo={postId} type="reply" />
				</div>

				{#if (post.current.replyIds?.length || 0) > 0}
					<hr />

					{#each post.current.replyIds || [] as postId}
						<div class="bleep-container">
							<Post {postId} />
						</div>
					{/each}
				{/if}
			</ScrollableContainer>
		{:else}
			<p>Loading...</p>
		{/if}
	</div>
</div>

<style lang="postcss">
	.page {
		display: flex;
		justify-content: center;

		width: 100%;

		margin-top: 16px;
	}

	hr {
		margin-bottom: 16px;
	}

	.page-content {
		width: 100%;
		max-width: 600px;
	}

	.bleep-container {
		margin-bottom: 16px;
	}

	.author {
		display: flex;
		align-items: center;

		gap: 8px;
	}

	.post-picture-container {
		display: flex;
		justify-content: center;
		width: 100%;

		max-height: 400px;

		img {
			object-fit: contain;
		}
	}
</style>
