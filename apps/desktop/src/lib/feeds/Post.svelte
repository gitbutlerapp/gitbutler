<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import Markdown from '$lib/components/Markdown.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { getPost, getPostAuthor } from '@gitbutler/shared/feeds/feedsPreview.svelte';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { UserService } from '@gitbutler/shared/users/userService';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { goto } from '$app/navigation';
	import Link from '$lib/shared/Link.svelte';

	type Props = {
		postId: string;
	};

	const { postId }: Props = $props();

	const feedService = getContext(FeedService);
	const appState = getContext(AppState);
	const userService = getContext(UserService);
	const projectService = getContext(ProjectService);
	const projectId = projectService.projectId;

	let postCardRef = $state<HTMLDivElement | undefined>(undefined);

	const post = $derived(getPost(appState, feedService, postId, { element: postCardRef }));
	const author = $derived(
		getPostAuthor(appState, feedService, userService, postId, { element: postCardRef })
	);
</script>

{#if post.current}
	<div bind:this={postCardRef}>
		<SectionCard>
			<div class="author">
				<Avatar
					size="medium"
					tooltip={author.current?.name || 'Unknown'}
					srcUrl={author.current?.avatarUrl || ''}
				/>
				<p>
					{#if post.current.postType === 'commit'}
						<b>{author.current?.name}</b> committed <Link href={'urmom'}
							>{post.current.metadata.commitSha.slice(0, 7)}</Link
						>
					{:else}
						<b>{author.current?.name}</b>
					{/if}
				</p>
			</div>

			<Markdown content={post.current.content} />

			{#if post.current.pictureUrl}
				<div class="post-picture-container">
					<img src={post.current.pictureUrl} alt="" referrerpolicy="no-referrer" />
				</div>
			{/if}

			<div class="post-actions">
				<Button
					onclick={() => {
						goto(`/${projectId}/feed/${postId}`);
					}}
					kind="soft">Replies: {post.current.replyIds?.length || 0}</Button
				>
			</div>
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

	.post-actions {
		display: flex;

		gap: 8px;
	}
</style>
