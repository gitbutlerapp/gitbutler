<script lang="ts">
	import BranchesCardTemplate from '$components/branchesPage/BranchesCardTemplate.svelte';
	import { type BranchListing, BranchListingDetails } from '$lib/branches/branchListing';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { TestId } from '$lib/testing/testIds';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
	import type { PullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		branchListing: BranchListing;
		prs: PullRequest[];
		selected: boolean;
		onclick: (args: { listing: BranchListing; pr?: PullRequest }) => void;
	}

	const { projectId, branchListing, prs, selected, onclick }: Props = $props();

	const unknownName = 'unknown';
	const unknownEmail = 'example@example.com';

	const userService = inject(USER_SERVICE);
	const gitConfigService = inject(GIT_CONFIG_SERVICE);
	const branchService = inject(BRANCH_SERVICE);

	const user = userService.user;

	// TODO: Use information from all PRs in a stack?
	const pr = $derived(prs.at(0));

	const branchDetailsResult = $derived(branchService.get(projectId, branchListing.name));

	let lastCommitDetails = $state<{ authorName: string; lastCommitAt?: Date }>();
	let branchListingDetails = $derived(branchDetailsResult?.current.data);

	// If there are zero commits we should not show the author
	const ownedByUser = $derived(branchListingDetails?.numberOfCommits === 0);

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

	$effect(() => {
		setAvatars(ownedByUser, branchListingDetails);
	});

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
</script>

<BranchesCardTemplate
	testId={TestId.BranchListCard}
	{selected}
	onclick={() => onclick?.({ listing: branchListing, pr })}
>
	{#snippet content()}
		<div class="sidebar-entry__header">
			<SeriesLabelsRow fontSize="13" series={filteredStackBranches} />
			{#if branchListing.stack?.inWorkspace}
				<div class="sidebar-entry__applied-tag">
					<span class="text-10 text-semibold">Workspace</span>
				</div>
			{/if}
		</div>

		<div class="text-12 sidebar-entry__about">
			{#if pr}
				<ReviewBadge
					type="PR"
					status={pr.draft ? 'draft' : 'unknown'}
					title={pr.title}
					number={pr.number}
				/>
				<span class="sidebar-entry__divider">•</span>
			{/if}

			{#if avatars}
				<AvatarGroup {avatars} />
				<span class="sidebar-entry__divider">•</span>
			{/if}

			{#each branchListing.remotes as remote}
				<span>{remote}</span>
				<span class="sidebar-entry__divider">•</span>
			{/each}
			{#if branchListing.hasLocal}
				<span>local</span>
				<span class="sidebar-entry__divider">•</span>
			{/if}
			{#if branchListing.remotes.length === 0 && !branchListing.hasLocal}
				<span class="sidebar-entry__divider">•</span>
				<span>No remotes</span>
			{/if}
		</div>
	{/snippet}
	{#snippet details()}
		<div class="text-12 sidebar-entry__details">
			<span class="truncate">
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

		&:last-child {
			display: none;
		}
	}

	.sidebar-entry__applied-tag {
		display: flex;
		padding: 2px 4px;
		border-radius: 10px;
		background-color: var(--clr-scale-ntrl-50);
		color: var(--clr-theme-ntrl-on-element);
	}

	.sidebar-entry__details {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		gap: 6px;
	}

	.sidebar-entry__details-item {
		display: flex;
		align-items: center;
		gap: 5px;
		color: var(--clr-text-2);
	}
</style>
