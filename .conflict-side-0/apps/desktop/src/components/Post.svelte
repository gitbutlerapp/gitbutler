<script lang="ts">
	import { ProjectService } from '$lib/project/projectService';
	import { getContext } from '@gitbutler/shared/context';
	import { getPostAuthor } from '@gitbutler/shared/feeds/feedsPreview.svelte';
	import { postsSelectors } from '@gitbutler/shared/feeds/postsSlice';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { registerInterestInView } from '@gitbutler/shared/interest/registerInterestFunction.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { UserService } from '@gitbutler/shared/users/userService';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		postId: string;
	};

	const { postId }: Props = $props();

	const feedService = getContext(FeedService);
	const appState = getContext(AppState);
	const userService = getContext(UserService);
	const projectService = getContext(ProjectService);
	const projectId = projectService.projectId;

	// Register interest for posts
	$effect(() => {
		const interest = feedService.getPostWithRepliesInterest(postId);
		registerInterestInView(interest, postCardRef);
	});
	const post = $derived(postsSelectors.selectById(appState.posts, postId));
	$inspect(post);

	const author = $derived(getPostAuthor(appState, feedService, userService, postId));

	let postCardRef = $state<HTMLDivElement | undefined>(undefined);
</script>

{#if post}
	<div bind:this={postCardRef}>
		<SectionCard>
			<div class="author">
				<Loading loadable={author?.current}>
					{#snippet children(author)}
						<Avatar
							size="medium"
							tooltip={author.name || 'Unknown'}
							srcUrl={author.avatarUrl || ''}
						/>
						<p>{author.name}</p>
					{/snippet}
				</Loading>
			</div>

			<Markdown content={post.content} />

			{#if post.pictureUrl}
				<div class="post-picture-container">
					<img src={post.pictureUrl} alt="" referrerpolicy="no-referrer" />
				</div>
			{/if}

			{#if post.replyIds}
				<div>
					<Button
						onclick={() => {
							goto(`/${projectId}/feed/${postId}`);
						}}
						kind="outline">Replies: {post.replyIds.length}</Button
					>
				</div>
			{:else}
				<p>Loading...</p>
			{/if}
		</SectionCard>
	</div>
{:else}
	<p>Loading...</p>
{/if}

<style lang="postcss">
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
