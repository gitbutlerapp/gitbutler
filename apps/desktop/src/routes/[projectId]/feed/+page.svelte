<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import Post from '$lib/feeds/Post.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { getFeed, getFeedLastPost } from '@gitbutler/shared/feeds/feedsPreview.svelte';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import {
		getFeedIdentityForRepositoryId,
		getParentForRepositoryId
	} from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';

	const appState = getContext(AppState);
	const feedService = getContext(FeedService);
	const projectService = getContext(ProjectService);
	const project = projectService.project;
	const cloudProjectService = getContext(CloudProjectService);

	const parentProject = $derived(
		$project?.api
			? getParentForRepositoryId(appState, cloudProjectService, $project.api.repository_id)
			: undefined
	);

	const feedIdentity = $derived(
		$project?.api
			? getFeedIdentityForRepositoryId(appState, cloudProjectService, $project.api.repository_id)
			: undefined
	);

	const feed = $derived(getFeed(appState, feedService, feedIdentity?.current));

	// Post creation
	let newPostContent = $state('');
	function createPost() {
		if (!feedIdentity?.current) return;
		if (!parentProject?.current) return;

		feedService.createPost(
			newPostContent,
			parentProject.current.repositoryId,
			feedIdentity.current
		);
		newPostContent = '';
	}

	// Infinite scrolling
	const lastPost = $derived(getFeedLastPost(appState, feedService, feed.current));

	let lastElement = $state<HTMLElement | undefined>();
	$effect(() => {
		if (!lastElement) return;

		const observer = new IntersectionObserver((entries) => {
			if (entries[0]?.isIntersecting && lastPost.current?.createdAt && feedIdentity?.current) {
				feedService.getFeedPage(feedIdentity.current, lastPost.current.createdAt);
			}
		});

		observer.observe(lastElement);
		return () => observer.disconnect();
	});
</script>

<ScrollableContainer>
	<div>
		<input type="text" bind:value={newPostContent} />
		<Button onclick={createPost}>Create</Button>
	</div>

	<div>
		{#if feed.current}
			{#each feed.current.postIds as postId, index (postId)}
				{#if index < feed.current.postIds.length - 1 && lastPost.current && feedIdentity?.current}
					<div bind:this={lastElement}></div>
				{/if}

				<Post {postId} />
			{/each}
		{/if}
	</div>
</ScrollableContainer>
