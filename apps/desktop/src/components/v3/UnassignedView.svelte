<script lang="ts">
	import UnassignedFoldButton from '$components/v3/UnassignedFoldButton.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { IntelligentScrollingService } from '$lib/intelligentScrolling/service';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';

	import Button from '@gitbutler/ui/Button.svelte';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		focus: DefinedFocusable;
	}

	const { projectId, focus }: Props = $props();

	const selectionId = { type: 'worktree', stackId: undefined } as SelectionId;

	const [uiState, uncommittedService, intelligentScrollingService] = inject(
		UiState,
		UncommittedService,
		IntelligentScrollingService
	);
	const projectState = $derived(uiState.project(projectId));
	const unassignedSidebaFolded = $derived(uiState.global.unassignedSidebaFolded);
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(exclusiveAction?.type === 'commit');
	let isScrollable = $state<boolean>(false);

	const treeChanges = $derived(uncommittedService.changesByStackId(null));
	const treeChangesCount = $derived(treeChanges.current.length);
	const changesToCommit = $derived(treeChangesCount > 0);
	let foldedContentWidth = $state<number>(0);

	function unfoldView() {
		unassignedSidebaFolded.set(false);
	}

	$effect(() => {
		if (isCommitting && changesToCommit) {
			unassignedSidebaFolded.set(false);
		}
	});
</script>

{#if !unassignedSidebaFolded.current}
	<div class="unassigned">
		<WorktreeChanges
			title="Unassigned"
			{projectId}
			stackId={undefined}
			active={selectionId.type === 'worktree' &&
				selectionId.stackId === undefined &&
				focus === DefinedFocusable.ViewportLeft}
			onscrollexists={(exists: boolean) => {
				isScrollable = exists;
			}}
			overflow
			onselect={() => {
				intelligentScrollingService.unassignedFileClicked(projectId);
			}}
		>
			{#snippet emptyPlaceholder()}
				<div class="unassigned__empty">
					<div class="unassigned__empty__placeholder">
						{@html noChanges}
						<p class="text-13 text-body unassigned__empty__placeholder-text">
							You're all caught up!<br />
							No files need committing
						</p>
					</div>
					<WorktreeTipsFooter />
				</div>
			{/snippet}
		</WorktreeChanges>

		{#if changesToCommit}
			<div class="create-new" class:sticked-bottom={isScrollable}>
				<Button
					type="button"
					wide
					disabled={isCommitting}
					onclick={() => {
						projectState.exclusiveAction.set({ type: 'commit' });
						uncommittedService.checkAll(null);
					}}
					icon={isCommitting ? undefined : 'plus-small'}
					testId={TestId.CommitToNewBranchButton}
					kind="outline"
				>
					{#if isCommitting}
						Committing…
					{:else}
						Commit to new branch
					{/if}
				</Button>
			</div>
		{/if}
	</div>
{:else}
	<div
		role="presentation"
		class="unassigned-folded"
		ondblclick={unfoldView}
		class:changes-to-commit={changesToCommit}
	>
		<UnassignedFoldButton active={true} onclick={unfoldView} />

		<div class="unassigned-folded__content">
			<Badge>
				{treeChangesCount > 99 ? '99+' : treeChangesCount}
			</Badge>
			<span
				bind:clientWidth={foldedContentWidth}
				style="height: {foldedContentWidth}px;"
				class="unassigned-folded__text text-14 text-semibold">Unassigned</span
			>
		</div>
	</div>
{/if}

<style lang="postcss">
	.unassigned__empty {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.unassigned__empty__placeholder {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 0 20px 40px;
		gap: 20px;
	}

	.unassigned__empty__placeholder-text {
		color: var(--clr-text-3);
		text-align: center;
	}

	.unassigned {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);
	}

	.create-new {
		display: flex;
		padding: 12px 12px 14px 12px;
		background-color: var(--clr-bg-1);
	}

	/* FOLDED */
	.unassigned-folded {
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		padding: 11px 0;
		gap: 10px;

		&.changes-to-commit {
			background-color: var(--clr-bg-1);
		}
	}

	.unassigned-folded__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		gap: 10px;
	}

	.unassigned-folded__text {
		display: flex;
		align-items: center;
		margin-right: 2px;
		transform: rotate(-90deg);
	}

	/* MODIFIERS */
	.sticked-bottom {
		border-top: 1px solid var(--clr-border-2);
	}
</style>
