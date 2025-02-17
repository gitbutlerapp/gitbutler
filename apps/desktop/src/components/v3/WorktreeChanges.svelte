<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { commitPath, DesktopRoutesService } from '$lib/routes/routes.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId?: string;
	};

	const { projectId, stackId }: Props = $props();

	const changeSelection = getContext(ChangeSelectionService);
	const worktreeService = getContext(WorktreeService);
	const desktopRouteService = getContext(DesktopRoutesService);
	createCommitStore(undefined);

	const changesQuery = $derived(worktreeService.getChanges(projectId));
	const disabled = $derived(!!desktopRouteService.isCommitPath);

	/** Clear any selected changes that no longer exist. */
	$effect(() => {
		const affectedPaths = changesQuery.current.data?.map((c) => c.path);
		changeSelection.retain(affectedPaths);
	});
</script>

<div class="worktree-header">
	<div class="text-14 text-semibold">Uncommitted changes</div>
	<Button kind="ghost" icon="sidebar-unfold" />
</div>

<ReduxResult result={changesQuery.current}>
	{#snippet children(changes)}
		<div class="uncommitted-changes">
			<FileList {projectId} {changes} showCheckboxes={disabled} />
			<div class="start-commit">
				<Button
					kind={disabled ? 'outline' : 'solid'}
					type="button"
					size="cta"
					wide
					{disabled}
					onclick={() => {
						if (stackId) {
							goto(commitPath(projectId, stackId));
						}
					}}
				>
					Start a commit…
				</Button>
			</div>
		</div>
	{/snippet}
	{#snippet empty()}
		<div class="text-12 text-body helper-text">
			{@html noChanges}
			<div>You're all caught up!</div>
			<div>No files need committing</div>
		</div>
	{/snippet}
</ReduxResult>

<style>
	.worktree-header {
		width: 100%;
		display: flex;

		align-items: center;
		justify-content: space-between;
		padding: 10px 8px 10px 14px;
		text-wrap: nowrap;
		overflow: hidden;

		& > div {
			width: 100%;
			overflow: hidden;
			white-space: nowrap;
			text-overflow: ellipsis;
		}
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
