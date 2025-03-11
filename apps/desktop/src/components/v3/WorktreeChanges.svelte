<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [changeSelection, worktreeService, uiState] = inject(
		ChangeSelectionService,
		WorktreeService,
		UiState
	);

	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage.get());
	const isCommitting = $derived(drawerPage.current === 'new-commit');

	// TODO: Make this go away.
	createCommitStore(undefined);

	const changesResult = $derived(worktreeService.getChanges(projectId));

	/** Clear any selected changes that no longer exist. */
	$effect(() => {
		const affectedPaths = changesResult.current.data?.map((c) => c.path);
		changeSelection.retain(affectedPaths);
	});
</script>

<ReduxResult result={changesResult.current}>
	{#snippet children(changes)}
		<div class="worktree-header text-14 text-semibold">
			<span>Uncommitted changes</span>
			<Badge>{changes.length}</Badge>
		</div>
		{#if changes.length > 0}
			<div class="uncommitted-changes">
				<FileList {projectId} {changes} showCheckboxes={isCommitting} />
				<div class="start-commit">
					<Button
						kind={isCommitting ? 'outline' : 'solid'}
						type="button"
						size="cta"
						wide
						disabled={isCommitting}
						onclick={() => projectState.drawerPage.set('new-commit')}
					>
						Start a commit…
					</Button>
				</div>
			</div>
		{:else}
			<div class="text-12 text-body helper-text">
				{@html noChanges}
				<div>You're all caught up!</div>
				<div>No files need committing</div>
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style>
	.worktree-header {
		display: flex;
		padding: 10px 8px 10px 14px;
		width: 100%;
		gap: 4px;
		align-items: center;
		text-wrap: nowrap;
		overflow: hidden;
	}

	.uncommitted-changes {
		display: flex;
		flex: 1;
		width: 100%;
		height: 100%;
		display: flex;
		justify-items: top;
		flex-direction: column;
		align-items: top;
		justify-content: top;
		overflow: hidden;
	}

	.helper-text {
		text-align: center;
		color: var(--clr-text-2);
		opacity: 0.6;
		margin-top: 10px;
	}

	.start-commit {
		padding: 16px;
	}
</style>
