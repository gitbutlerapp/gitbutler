<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import newBranchSmolSVG from '$lib/assets/empty-state/new-branch-smol.svg?raw';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	interface Props {
		projectId: string;
		branchName: string;
		stackId?: string;
	}

	const { projectId, stackId, branchName }: Props = $props();

	const [stackService] = inject(StackService);

	const branchDetailsResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName)
	);
	const changesResult = $derived(stackService.branchChanges(projectId, undefined, branchName));

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabTrigger = $state<HTMLButtonElement>();
	let isContextMenuOpen = $state(false);
</script>

<ReduxResult {projectId} result={branchDetailsResult.current}>
	{#snippet children(branchDetails, { stackId, projectId })}
		{@const hasCommits = branchDetails.commits.length > 0}
		{@const remoteTrackingBranch = branchDetails.remoteTrackingBranch}
		<Drawer {projectId} {stackId}>
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
					<h3 class="text-15 text-bold truncate">{branchDetails.name}</h3>
				</div>
			{/snippet}

			{#snippet kebabMenu()}
				<Button
					size="tag"
					icon="kebab"
					kind="ghost"
					activated={isContextMenuOpen}
					bind:el={kebabTrigger}
					onclick={() => {
						contextMenu?.toggle();
					}}
				/>
			{/snippet}

			{#if hasCommits}
				<div class="unapplied-branch-view">
					<div class="unapplied-branch-view__header-container">
						<div class="text-12 unapplied-branch-view__header-details-row">
							<BranchBadge pushStatus={branchDetails.pushStatus} />
							<span class="unapplied-branch-view__details-divider">•</span>

							{#if branchDetails.isConflicted}
								<span class="unapplied-branch-view__header-details-row-conflict">Has conflicts</span
								>
								<span class="unapplied-branch-view__details-divider">•</span>
							{/if}

							<span>Contribs:</span>
							<AvatarGroup
								maxAvatars={2}
								avatars={branchDetails.authors.map((a) => ({
									name: a.name,
									srcUrl: a.gravatarUrl
								}))}
							/>

							<span class="unapplied-branch-view__details-divider">•</span>

							<span class="truncate">{getTimeAgo(new Date(branchDetails.lastUpdatedAt))}</span>
						</div>
					</div>
				</div>
			{:else}
				<div class="unapplied-branch-view__empty-state">
					<div class="unapplied-branch-view__empty-state__image">
						{@html newBranchSmolSVG}
					</div>
					<h3 class="text-18 text-semibold unapplied-branch-view__empty-state__title">
						This is a new branch
					</h3>
					<p class="text-13 text-body unapplied-branch-view__empty-state__description">
						Commit your changes here. You can stack additional branches or apply them independently.
						You can also drag and drop files to start a new commit.
					</p>
				</div>
			{/if}

			{#snippet filesSplitView()}
				<ReduxResult {projectId} result={changesResult.current}>
					{#snippet children(changes, env)}
						<ChangedFiles
							title="All changed files"
							projectId={env.projectId}
							stackId={env.stackId}
							selectionId={{ type: 'branch', branchName }}
							{changes}
						/>
					{/snippet}
				</ReduxResult>
			{/snippet}
		</Drawer>
	{/snippet}
</ReduxResult>

<style>
	.unapplied-branch-view {
		display: flex;
		flex-direction: column;
		gap: 16px;
		height: 100%;
	}

	.branch__header {
		display: flex;
		align-items: center;
		gap: 8px;
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

	.unapplied-branch-view__header-container {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
		overflow: hidden;
	}

	.unapplied-branch-view__header-details-row {
		width: 100%;
		color: var(--clr-text-2);
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.unapplied-branch-view__header-details-row-conflict {
		color: var(--clr-theme-err-element);
	}

	.unapplied-branch-view__details-divider {
		color: var(--clr-text-3);
	}

	/* EMPTY STATE */
	.unapplied-branch-view__empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		padding: 30px;
		max-width: 540px;
		margin: 0 auto;
	}

	.unapplied-branch-view__empty-state__image {
		width: 180px;
		margin-bottom: 20px;
	}

	.unapplied-branch-view__empty-state__title {
		margin-bottom: 10px;
	}

	.unapplied-branch-view__empty-state__description {
		color: var(--clr-text-2);
		text-wrap: balance;
	}
</style>
