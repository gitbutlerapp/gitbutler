<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import {
		BranchListingDetails,
		BranchListingService,
		type BranchListing
	} from '$lib/branches/branchListing';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getContext } from '$lib/utils/context';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import AvatarGrouping from '@gitbutler/ui/avatar/AvatarGrouping.svelte';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
	import SidebarEntry from '@gitbutler/ui/sidebarEntry/SidebarEntry.svelte';
	import type { Readable } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		branchListing: BranchListing;
	}

	const { branchListing }: Props = $props();

	const branchListingService = getContext(BranchListingService);
	const project = getContext(Project);

	const gitHostListingService = getGitHostListingService();
	const prs = $derived($gitHostListingService?.prs);
	const pr = $derived($prs?.find((pr) => pr.sourceBranch === branchListing.name));

	let branchListingDetails = $state<Readable<BranchListingDetails | undefined>>();

	let hasBeenSeen = $state(false);

	$effect(() => {
		if (hasBeenSeen) {
			updateBranchListingDetails(branchListing.name);
		}
	});

	function updateBranchListingDetails(branchName: string) {
		branchListingDetails = branchListingService.getBranchListingDetails(branchName);
	}

	function onMouseDown() {
		goto(formatBranchURL(project, branchListing.name));
	}

	const selected = $derived($page.url.pathname === formatBranchURL(project, branchListing.name));

	function formatBranchURL(project: Project, name: string) {
		return `/${project.id}/branch/${encodeURIComponent(name)}`;
	}
</script>

<SidebarEntry
	title={branchListing.name}
	remotes={branchListing.remotes}
	local={branchListing.hasLocal}
	applied={branchListing.virtualBranch?.inWorkspace}
	lastCommitDetails={{
		authorName: branchListing.lastCommiter.name || 'Unknown',
		lastCommitAt: branchListing.updatedAt
	}}
	pullRequestDetails={pr && {
		title: pr.title
	}}
	branchDetails={$branchListingDetails && {
		commitCount: $branchListingDetails.numberOfCommits,
		linesAdded: $branchListingDetails.linesAdded,
		linesRemoved: $branchListingDetails.linesRemoved
	}}
	onFirstSeen={() => (hasBeenSeen = true)}
	{onMouseDown}
	{selected}
>
	{#snippet authorAvatars()}
		<AvatarGrouping>
			{#if $branchListingDetails}
				{#each $branchListingDetails.authors as author}
					{#await gravatarUrlFromEmail(author.email || 'example@example.com') then gravatarUrl}
						<Avatar
							srcUrl={gravatarUrl}
							size="medium"
							tooltipText={author.name || 'unknown'}
							altText={author.name || 'unknown'}
						/>
					{/await}
				{/each}
			{/if}
		</AvatarGrouping>
	{/snippet}
</SidebarEntry>
