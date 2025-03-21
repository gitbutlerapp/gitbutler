<script lang="ts">
	import BranchReview from '$components/BranchReview.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
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
			<Drawer {projectId} {stackId}>
				{#snippet header()}
					<div class="branch-view__header-title-row">
						<h3 class="text-15 text-bold">
							{branch.name}
						</h3>
					</div>
				{/snippet}

				<div class="branch-view">
					<div class="branch-view__header-container">
						<div class="text-13 branch-view__header-details-row">
							<BranchBadge pushStatus={branchDetails.pushStatus} />

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

							<span>Updated {getTimeAgo(new Date(branchDetails.lastUpdatedAt))}</span>

							<span class="branch-view__details-divider">•</span>
						</div>
					</div>

					<BranchReview openForgePullRequest={() => {}} {stackId} {projectId} {branchName} />

					<ChangedFiles type="branch" {projectId} {stackId} {branchName} />
				</div>
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
	}

	.branch-view__header-container {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
	}

	.branch-view__header-title-row {
		display: flex;
		align-items: center;
		gap: 6px;
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
		opacity: 0.4;
	}
</style>
