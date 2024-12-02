<script lang="ts">
	import Markdown from '$lib/components/Markdown.svelte';
	import CreatePost from '$lib/feeds/CreatePost.svelte';
	import Post from '$lib/feeds/Post.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { getPostAuthor } from '@gitbutler/shared/feeds/feedsPreview.svelte';
	import { postsSelectors } from '@gitbutler/shared/feeds/postsSlice';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { registerInterest } from '@gitbutler/shared/interest/registerInterestFunction.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { UserService } from '@gitbutler/shared/users/userService';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { page } from '$app/stores';

	const feedService = getContext(FeedService);
	const appState = getContext(AppState);
	const userService = getContext(UserService);

	const postId = $derived($page.params.postId);

	// Register interest for posts
	$effect(() => {
		if (!postId) return;
		const interest = feedService.getPostWithRepliesInterest(postId);
		registerInterest(interest);
	});
	const post = $derived(postId ? postsSelectors.selectById(appState.posts, postId) : undefined);
	const author = $derived(
		postId ? getPostAuthor(appState, feedService, userService, postId) : undefined
	);
</script>

<div class="page">
	<div class="page-content">
		{#if post}
			<ScrollableContainer wide>
				<div class="bleep-container">
					<Button
						onclick={() => {
							history.back();
						}}>Back</Button
					>
				</div>

				<div class="bleep-container">
					<SectionCard>
						<div class="author">
							<Avatar
								size="medium"
								tooltip={author?.current?.name || 'Unknown'}
								srcUrl={author?.current?.avatarUrl || ''}
							/>
							<p>{author?.current?.name}</p>
						</div>

						<Markdown content={post.content} />
					</SectionCard>
				</div>

				<hr />

				<div class="bleep-container">
					<CreatePost replyTo={postId} />
				</div>

				{#if (post.replyIds?.length || 0) > 0}
					<hr />

					{#each post.replyIds || [] as postId}
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
</style>
