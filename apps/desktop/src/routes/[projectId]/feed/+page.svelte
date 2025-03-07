<script lang="ts">
	import CreatePost from '$components/CreatePost.svelte';
	import Post from '$components/Post.svelte';
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { getContext } from '@gitbutler/shared/context';
	import { getFeed, getFeedLastPost } from '@gitbutler/shared/feeds/feedsPreview.svelte';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { getFeedIdentityForRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	const appState = getContext(AppState);
	const feedService = getContext(FeedService);
	const projectService = getContext(ProjectService);
	const project = projectService.project;

	const feedIdentity = $derived(
		$project?.api ? getFeedIdentityForRepositoryId($project.api.repository_id) : undefined
	);

	const feed = $derived.by(() => {
		if (feedIdentity?.current.status !== 'found') return;
		return getFeed(appState, feedService, feedIdentity?.current.value);
	});

	// Infinite scrolling
	const lastPost = $derived(getFeedLastPost(appState, feedService, feed?.current));

	let lastElement = $state<HTMLElement | undefined>();
	$effect(() => {
		if (!lastElement) return;

		const observer = new IntersectionObserver((entries) => {
			if (
				entries[0]?.isIntersecting &&
				lastPost.current?.createdAt &&
				feedIdentity?.current.status === 'found'
			) {
				feedService.getFeedPage(feedIdentity.current.value, lastPost.current.createdAt);
			}
		});

		observer.observe(lastElement);
		return () => observer.disconnect();
	});
</script>

<div class="page">
	<div class="page-content">
		<ScrollableContainer>
			<div class="bleep-container">
				<CreatePost />
			</div>

			<hr />

			{#if feed?.current}
				{#each feed.current.postIds as postId, index (postId)}
					<div class="bleep-container">
						{#if index < feed.current.postIds.length - 1 && lastPost.current && feedIdentity?.current}
							<div bind:this={lastElement}></div>
						{/if}

						<Post {postId} />
					</div>
				{/each}
			{/if}
		</ScrollableContainer>
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
</style>
