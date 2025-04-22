<script lang="ts">
	import BranchesCardTemplate from '$components/v3/branchesPage/BranchesCardTemplate.svelte';
	import { BranchListingDetails, type BranchListing } from '$lib/branches/branchListing';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { GitConfigService } from '$lib/config/gitConfigService';
	import { Project } from '$lib/project/project';
	import { UserService } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	// import SidebarEntry from '@gitbutler/ui/SidebarEntry.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
	import type { PullRequest } from '$lib/forge/interface/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	interface Props {
		projectId: string;
		branchListing: BranchListing;
		prs: PullRequest[];
	}

	const { projectId, branchListing, prs }: Props = $props();

	const unknownName = 'unknown';
	const unknownEmail = 'example@example.com';

	const [userService, gitConfigService, project, branchService] = inject(
		UserService,
		GitConfigService,
		Project,
		BranchService
	);

	const user = userService.user;

	// TODO: Use information from all PRs in a stack?
	const pr = $derived(prs.at(0));

	let hasBeenSeen = $state(false);

	const branchDetailsResult = $derived(
		hasBeenSeen ? branchService.get(projectId, branchListing.name) : undefined
	);

	let lastCommitDetails = $state<{ authorName: string; lastCommitAt?: Date }>();
	let branchListingDetails = $derived(branchDetailsResult?.current.data);

	// If there are zero commits we should not show the author
	const ownedByUser = $derived(branchListingDetails?.numberOfCommits === 0);

	function handleClick() {
		if (branchListing.stack?.inWorkspace) {
			goto(`/${project.id}/board`);
		} else {
			goto(formatBranchURL(project, branchListing.name));
		}
	}

	const selected = $derived(page.url.pathname === formatBranchURL(project, branchListing.name));

	function formatBranchURL(project: Project, name: string) {
		return `/${project.id}/branch/${encodeURIComponent(name)}`;
	}

	$effect(() => {
		let canceled = false;

		if (ownedByUser) {
			gitConfigService.get('user.name').then((userName) => {
				if (canceled) return;

				if (userName) {
					lastCommitDetails = { authorName: userName };
				} else {
					lastCommitDetails = undefined;
				}
			});
		} else {
			lastCommitDetails = {
				authorName: branchListing.lastCommiter.name || unknownName,
				lastCommitAt: new Date(branchListing.updatedAt)
			};
		}
	});

	let avatars = $state<{ name: string; srcUrl: string }[]>([]);

	async function setAvatars(ownedByUser: boolean, branchListingDetails?: BranchListingDetails) {
		if (ownedByUser) {
			const name = (await gitConfigService.get('user.name')) || unknownName;
			const email = (await gitConfigService.get('user.email')) || unknownEmail;
			const srcUrl =
				email.toLowerCase() === $user?.email?.toLowerCase() && $user?.picture
					? $user?.picture
					: await gravatarUrlFromEmail(email);

			avatars = [{ name, srcUrl: srcUrl }];
		} else if (branchListingDetails) {
			avatars = branchListingDetails.authors
				? await Promise.all(
						branchListingDetails.authors.map(async (author) => {
							return {
								name: author.name || unknownName,
								srcUrl:
									(author.email?.toLowerCase() === $user?.email?.toLowerCase()
										? $user?.picture
										: author.gravatarUrl) ??
									(await gravatarUrlFromEmail(author.email || unknownEmail))
							};
						})
					)
				: [];
		} else {
			avatars = [];
		}
	}

	const stackBranches = $derived(branchListing.stack?.branches);
	const filteredStackBranches = $derived(
		stackBranches && stackBranches.length > 0 ? stackBranches : [branchListing.name]
	);

	const pullRequestDetails = $derived(
		pr && {
			title: pr.title,
			draft: pr.draft,
			number: pr.number
		}
	);

	$effect(() => {
		setAvatars(ownedByUser, branchListingDetails);
	});
</script>

<BranchesCardTemplate {selected} onclick={handleClick}>
	{#snippet content()}
		<div class="sidebar-entry__header">
			<SeriesLabelsRow series={filteredStackBranches} />
			{#if branchListing.stack?.inWorkspace}
				<div class="sidebar-entry__applied-tag">
					<span class="text-10 text-semibold">Workspace</span>
				</div>
			{/if}
		</div>

		<div class="text-12 sidebar-entry__about">
			{#if pullRequestDetails}
				<ReviewBadge
					prStatus={pullRequestDetails.draft ? 'draft' : 'unknown'}
					prTitle={pullRequestDetails.title}
					prNumber={pullRequestDetails.number}
				/>
				<span class="sidebar-entry__divider">•</span>
			{/if}

			<AvatarGroup {avatars} />

			<!-- NEED API -->
			{#each branchListing.remotes as remote}
				<span class="sidebar-entry__divider">•</span>
				<span>{remote}</span>
			{/each}
			{#if branchListing.hasLocal}
				<span class="sidebar-entry__divider">•</span>
				<span>local</span>
			{/if}
			{#if branchListing.remotes.length === 0 && !branchListing.hasLocal}
				<span class="sidebar-entry__divider">•</span>
				<span>No remotes</span>
			{/if}
		</div>
	{/snippet}
	{#snippet details()}
		<div class="text-12 sidebar-entry__details">
			<span>
				{#if lastCommitDetails}
					<TimeAgo date={lastCommitDetails.lastCommitAt} addSuffix />
					by {lastCommitDetails.authorName}
				{/if}
			</span>

			{#if branchListingDetails}
				<div class="sidebar-entry__details-item">
					{#if branchListingDetails.linesAdded}
						<span>
							+{branchListingDetails.linesAdded}
						</span>
					{/if}
					{#if branchListingDetails.linesRemoved}
						<span>
							-{branchListingDetails.linesRemoved}
						</span>
					{/if}

					<svg
						width="14"
						height="12"
						viewBox="0 0 14 12"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M10 6C10 7.65685 8.65685 9 7 9C5.34315 9 4 7.65685 4 6M10 6C10 4.34315 8.65685 3 7 3C5.34315 3 4 4.34315 4 6M10 6H14M4 6H0"
							stroke="currentColor"
						/>
					</svg>

					<span>{branchListingDetails?.numberOfCommits}</span>
				</div>
			{/if}
		</div>
	{/snippet}
</BranchesCardTemplate>

<!-- <SidebarEntry
	series={filteredStackBranches}
	remotes={branchListing.remotes}
	local={branchListing.hasLocal}
	applied={branchListing.stack?.inWorkspace}
	{lastCommitDetails}
	pullRequestDetails={pr && {
		title: pr.title,
		draft: pr.draft
	}}
	branchDetails={branchListingDetails && {
		commitCount: branchListingDetails.numberOfCommits,
		linesAdded: branchListingDetails.linesAdded,
		linesRemoved: branchListingDetails.linesRemoved
	}}
	onFirstSeen={() => (hasBeenSeen = true)}
	{onMouseDown}
	{selected}
	{avatars}
/> -->

<style lang="postcss">
	.sidebar-entry__about {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
	}

	.sidebar-entry__header {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.sidebar-entry__divider {
		color: var(--clr-text-3);
	}

	.sidebar-entry__applied-tag {
		display: flex;
		background-color: var(--clr-scale-ntrl-50);
		padding: 2px 4px;
		border-radius: 10px;
		color: var(--clr-theme-ntrl-on-element);
	}

	.sidebar-entry__details {
		display: flex;
		gap: 6px;
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}

	.sidebar-entry__details-item {
		display: flex;
		gap: 5px;
		align-items: center;
		color: var(--clr-text-2);
	}
</style>
