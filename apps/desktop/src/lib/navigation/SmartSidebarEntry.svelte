<script lang="ts">
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

	interface Props {
		branchListing: BranchListing;
	}

	const { branchListing }: Props = $props();

	const gitHostListingService = getGitHostListingService();
	const branchListingService = getContext(BranchListingService);

	const prs = $derived($gitHostListingService?.prs);
	const pr = $derived($prs?.find((pr) => pr.sourceBranch === branchListing.name));

	let branchListingDetails = $state<BranchListingDetails>();

	async function onFirstSeen() {
		if (!branchListingDetails) {
			console.log('doing the math');
			branchListingDetails = await branchListingService.getBranchListingDetails(branchListing.name);
		}
	}
</script>

<SidebarEntry
	title={branchListing.name}
	remotes={branchListing.remotes}
	local={false}
	applied={!!branchListing.virtualBranch}
	lastCommitDetails={{
		authorName: branchListing.lastCommiter.name || 'Unknown',
		lastCommitAt: branchListing.updatedAt
	}}
	pullRequestDetails={pr && {
		title: pr.title
	}}
	branchDetails={branchListingDetails && {
		commitCount: branchListing.numberOfCommits,
		linesAdded: branchListingDetails.linesAdded,
		linesRemoved: branchListingDetails.linesRemoved
	}}
	{onFirstSeen}
>
	{#snippet authorAvatars()}
		<AvatarGrouping>
			{#each branchListing.authors as author}
				{#await gravatarUrlFromEmail(author.email || 'example@example.com') then gravatarUrl}
					<Avatar
						srcUrl={gravatarUrl}
						size="medium"
						tooltipText={author.name || 'unknown'}
						altText={author.name || 'unknown'}
					/>
				{/await}
			{/each}
		</AvatarGrouping>
	{/snippet}
</SidebarEntry>
