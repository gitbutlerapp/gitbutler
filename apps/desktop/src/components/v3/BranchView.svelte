<script lang="ts">
	import BranchReview from '$components/BranchReview.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import newBranchSmolSVG from '$lib/assets/empty-state/new-branch-smol.svg?raw';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UserService } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	interface Props {
		stackId: string;
		projectId: string;
		branchName: string;
	}

	const { stackId, projectId, branchName }: Props = $props();

	const [stackService, userService] = inject(StackService, UserService);
	const user = $derived(userService.user);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchCommitsResult = $derived(stackService.commits(projectId, stackId, branchName));

	function getGravatarUrl(email: string, existingGravatarUrl: string): string {
		if ($user?.email === undefined) {
			return existingGravatarUrl;
		}
		if (email === $user.email) {
			return $user.picture ?? existingGravatarUrl;
		}
		return existingGravatarUrl;
	}
</script>

{#if branchName}
	<ReduxResult result={combineResults(branchResult.current, branchDetailsResult.current)}>
		{#snippet children([branch, branchDetails])}
			<Drawer {projectId} {stackId} title={branch.name}>
				{#if branchCommitsResult.current.data && branchCommitsResult.current.data.length > 0}
					<div class="branch-view">
						<div class="branch-view__header-container">
							<div class="text-12 branch-view__header-details-row">
								<BranchBadge pushStatus={branchDetails.pushStatus} />
								<span class="branch-view__details-divider">•</span>

								{#if branchDetails.isConflicted}
									<span class="branch-view__header-details-row-conflict">Has conflicts</span>
									<span class="branch-view__details-divider">•</span>
								{/if}

								<span>Contributors:</span>
								<AvatarGroup
									maxAvatars={2}
									avatars={branchDetails.authors.map((a) => ({
										name: a.name,
										srcUrl: getGravatarUrl(a.email, a.gravatarUrl)
									}))}
								/>

								<span class="branch-view__details-divider">•</span>

								<span>{getTimeAgo(new Date(branchDetails.lastUpdatedAt))}</span>
							</div>
						</div>

						<BranchReview {stackId} {projectId} {branchName} />

						<ChangedFiles
							{projectId}
							{stackId}
							selectionId={{ type: 'branch', branchName, stackId }}
						/>
					</div>
				{:else}
					<div class="branch-view__empty-state">
						<div class="branch-view__empty-state__image">
							{@html newBranchSmolSVG}
						</div>
						<h3 class="text-18 text-semibold branch-view__empty-state__title">
							This is a new branch
						</h3>
						<p class="text-13 text-body branch-view__empty-state__description">
							Commit your changes here. You can stack additional branches or apply them
							independently. You can also drag and drop files to start a new commit.
						</p>
					</div>
				{/if}
			</Drawer>
		{/snippet}
	</ReduxResult>
{/if}

<style>
	.branch-view {
		display: flex;
		flex-direction: column;
		gap: 16px;
		align-self: stretch;
		height: 100%;
		overflow: hidden;
	}

	.branch-view__header-container {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
	}

	.branch-view__header-details-row {
		color: var(--clr-text-2);
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.branch-view__header-details-row-conflict {
		color: var(--clr-theme-err-element);
	}

	.branch-view__details-divider {
		color: var(--clr-text-3);
	}

	/* EMPTY STATE */
	.branch-view__empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		padding: 30px;
		max-width: 540px;
		margin: 0 auto;
	}

	.branch-view__empty-state__image {
		width: 180px;
		margin-bottom: 20px;
	}

	.branch-view__empty-state__title {
		margin-bottom: 10px;
	}

	.branch-view__empty-state__description {
		color: var(--clr-text-2);
		text-wrap: balance;
	}
</style>
