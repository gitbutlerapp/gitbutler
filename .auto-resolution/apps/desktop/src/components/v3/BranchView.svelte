<script lang="ts">
	import BranchReview from '$components/BranchReview.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import newBranchSmolSVG from '$lib/assets/empty-state/new-branch-smol.svg?raw';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { FocusManager } from '$lib/focus/focusManager.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { UserService } from '$lib/user/userService';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	interface Props {
		stackId: string;
		projectId: string;
		branchName: string;
	}

	const { stackId, projectId, branchName }: Props = $props();

	const [stackService, userService, uiState, focus, forge] = inject(
		StackService,
		UserService,
		UiState,
		FocusManager,
		DefaultForgeFactory
	);
	const stackState = $derived(uiState.stack(stackId));
	const focusedArea = $derived(focus.current);
	const user = $derived(userService.user);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchCommitsResult = $derived(stackService.commits(projectId, stackId, branchName));
	const forgeBranch = $derived(forge.current?.branch(branchName));

	$effect(() => {
		if (focusedArea === 'commit') {
			stackState.activeSelectionId.set({ type: 'branch', stackId, branchName });
		}
	});

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
	<ReduxResult
		{stackId}
		{projectId}
		result={combineResults(
			branchResult.current,
			branchDetailsResult.current,
			branchCommitsResult.current
		)}
	>
		{#snippet children([branch, branchDetails, branchCommits], { stackId, projectId })}
			{@const hasCommits = branchCommits.length > 0}
			{@const remoteTrackingBranch = branch.remoteTrackingBranch}
			<Drawer {projectId} {stackId} splitView={hasCommits}>
				{#snippet header()}
					<div class="branch__header">
						{#if hasCommits}
							<Tooltip
								text={remoteTrackingBranch
									? `Remote tracking branch:\n${remoteTrackingBranch}`
									: 'No remote tracking branch'}
							>
								<div class="remote-tracking-branch-icon" class:disabled={!remoteTrackingBranch}>
									<Icon
										name={remoteTrackingBranch ? 'remote-target-branch' : 'no-remote-target-branch'}
									/>
								</div>
							</Tooltip>
						{/if}
						<h3 class="text-15 text-bold truncate">{branch.name}</h3>
					</div>
				{/snippet}

				{#snippet extraActions()}
					{#if hasCommits}
						<Button
							size="tag"
							icon="copy-small"
							kind="outline"
							tooltip="Copy branch name"
							onclick={() => {
								writeClipboard(branch.name);
							}}
						/>

						{#if remoteTrackingBranch}
							<Button
								size="tag"
								icon="open-link"
								kind="outline"
								onclick={() => {
									const url = forgeBranch?.url;
									if (url) openExternalUrl(url);
								}}>Open in browser</Button
							>
						{/if}
					{/if}
				{/snippet}

				{#if hasCommits}
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

						<BranchReview {stackId} {projectId} branchName={branch.name} />
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

				{#snippet filesSplitView()}
					{#if hasCommits}
						<ChangedFiles
							{projectId}
							{stackId}
							selectionId={{ type: 'branch', branchName: branch.name, stackId }}
						/>
					{/if}
				{/snippet}
			</Drawer>
		{/snippet}
	</ReduxResult>
{/if}

<style>
	.branch-view {
		display: flex;
		flex-direction: column;
		gap: 16px;
		height: 100%;
	}

	.branch__header {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		overflow: hidden;
	}

	/*  */
	.remote-tracking-branch-icon {
		display: flex;
		gap: 6px;
		color: var(--clr-text-1);
		opacity: 0.5;
		transition: var(--transition-fast);

		&:hover {
			opacity: 0.7;
		}

		&.disabled {
			opacity: 0.3;
		}
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
