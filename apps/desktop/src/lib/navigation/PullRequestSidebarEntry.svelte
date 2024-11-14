<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import SidebarEntry from '@gitbutler/ui/SidebarEntry.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import type { PullRequest } from '$lib/forge/interface/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		pullRequest: PullRequest;
	}

	const { pullRequest }: Props = $props();

	const project = getContext(Project);

	const userService = getContext(UserService);
	const user = userService.user;

	const authorImgUrl =
		pullRequest.author?.email?.toLowerCase() === $user?.email?.toLowerCase()
			? $user?.picture
			: pullRequest.author?.gravatarUrl;

	function onMouseDown() {
		goto(formatPullRequestURL(project, pullRequest.number));
	}

	function formatPullRequestURL(project: Project, pullRequestNumber: number) {
		return `/${project.id}/pull/${pullRequestNumber}`;
	}

	const selected = $derived(
		$page.url.pathname === formatPullRequestURL(project, pullRequest.number)
	);
</script>

<SidebarEntry
	title={pullRequest.title}
	remotes={[]}
	local={false}
	applied={false}
	lastCommitDetails={{
		authorName: pullRequest.author?.name || 'Unknown',
		lastCommitAt: pullRequest.modifiedAt
	}}
	pullRequestDetails={pullRequest && {
		title: pullRequest.title,
		draft: pullRequest.draft
	}}
	{onMouseDown}
	{selected}
>
	{#snippet authorAvatars()}
		{#if authorImgUrl}
			<AvatarGroup
				avatars={[
					{
						srcUrl: authorImgUrl,
						name: pullRequest.author?.name || 'unknown'
					}
				]}
			/>
		{/if}
	{/snippet}
</SidebarEntry>
