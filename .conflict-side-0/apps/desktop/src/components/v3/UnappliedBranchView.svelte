<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchDetails from '$components/v3/BranchDetails.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	interface Props {
		projectId: string;
		branchName: string;
		stackId?: string;
	}

	const { projectId, stackId, branchName }: Props = $props();

	const [stackService] = inject(StackService);

	const branchResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName)
	);
	const changesResult = $derived(stackService.branchChanges(projectId, undefined, branchName));

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabTrigger = $state<HTMLButtonElement>();
	let isContextMenuOpen = $state(false);
</script>

<ReduxResult {projectId} result={branchResult.current}>
	{#snippet children(branch, { stackId, projectId })}
		{@const hasCommits = branch.commits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
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
					<h3 class="text-15 text-bold truncate">{branch.name}</h3>
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
				<BranchDetails {branch} />
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
</style>
