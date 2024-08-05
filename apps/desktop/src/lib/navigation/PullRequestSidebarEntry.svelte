<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import AvatarGrouping from '@gitbutler/ui/avatar/AvatarGrouping.svelte';
	import SidebarEntry from '@gitbutler/ui/sidebarEntry/SidebarEntry.svelte';
	import type { PullRequest } from '$lib/gitHost/interface/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		pullRequest: PullRequest;
	}

	const { pullRequest }: Props = $props();

	const project = getContext(Project);

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
		title: pullRequest.title
	}}
	{onMouseDown}
	{selected}
>
	{#snippet authorAvatars()}
		<AvatarGrouping>
			{#if pullRequest.author?.gravatarUrl}
				<Avatar
					srcUrl={pullRequest.author.gravatarUrl}
					size="medium"
					tooltipText={pullRequest.author.name || 'unknown'}
					altText={pullRequest.author.name || 'unknown'}
				/>
			{/if}
		</AvatarGrouping>
	{/snippet}
</SidebarEntry>
