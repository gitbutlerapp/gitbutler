<script lang="ts">
	import RulesList from '$components/RulesList.svelte';
	import UnassignedFoldButton from '$components/UnassignedFoldButton.svelte';
	import WorktreeChanges from '$components/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { stagingBehaviorFeature } from '$lib/config/uiFeatureFlags';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { INTELLIGENT_SCROLLING_SERVICE } from '$lib/intelligentScrolling/service';
	import { ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Badge, Button, TestId } from '@gitbutler/ui';

	interface Props {
		projectId: string;
	}

	const { projectId }: Props = $props();

	const selectionId = createWorktreeSelection({ stackId: undefined });

	const uiState = inject(UI_STATE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const intelligentScrollingService = inject(INTELLIGENT_SCROLLING_SERVICE);
	const idSelection = inject(ID_SELECTION);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = $derived(settingsService.appSettings);
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

	function unselectFiles() {
		idSelection.clear(selectionId);
	}

	$effect(() => {
		if (isCommitting && changesToCommit) {
			unassignedSidebaFolded.set(false);
		}
	});

	function foldUnnassignedView() {
		unassignedSidebaFolded.set(true);
	}

	function checkSelectedFilesForCommit() {
		const selectionId = createWorktreeSelection({});
		const selectedPaths = idSelection.values(selectionId).map((entry) => entry.path);

		// If there are selected paths in the unassigned selection, we check those.
		if (selectedPaths.length > 0) {
			uncommittedService.checkFiles(null, selectedPaths);
		} else {
			uncommittedService.checkAll(null);
		}
	}

	function uncheckAll() {
		uncommittedService.uncheckAll(null);
	}

	function checkAllFiles() {
		uncommittedService.checkAll(null);
	}

	function checkFilesForCommit(): true {
		switch ($stagingBehaviorFeature) {
			case 'all':
				checkAllFiles();
				return true;
			case 'selection':
				// We only check the selected files.
				checkSelectedFilesForCommit();
				return true;
			case 'none':
				uncheckAll();
				return true;
		}
	}
</script>

{#snippet foldButton()}
	{#if !isCommitting && !unassignedSidebaFolded.current}
		<div class="unassigned__fold-button">
			<UnassignedFoldButton active={false} onclick={foldUnnassignedView} />
		</div>
	{/if}
{/snippet}

{#if !unassignedSidebaFolded.current}
	<div class="unassigned">
		{#if $settingsStore?.featureFlags.rules}
			<RulesList {foldButton} {projectId} />
		{/if}

		<div role="presentation" class="unassigned__files" onclick={unselectFiles}>
			<WorktreeChanges
				title="Unassigned"
				{projectId}
				stackId={undefined}
				onscrollexists={(exists: boolean) => {
					isScrollable = exists;
				}}
				overflow
				mode="unassigned"
				onselect={() => {
					intelligentScrollingService.unassignedFileClicked(projectId);
				}}
				foldButton={$settingsStore?.featureFlags.rules ? undefined : foldButton}
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
		</div>

		{#if changesToCommit}
			<div class="create-new" class:sticked-bottom={isScrollable}>
				<Button
					type="button"
					wide
					disabled={!!projectState.exclusiveAction.current}
					onclick={() => {
						projectState.exclusiveAction.set({
							type: 'commit',
							stackId: undefined,
							branchName: undefined
						});
						checkFilesForCommit();
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
		use:focusable
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

	.unassigned__files {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	/* MODIFIERS */
	.sticked-bottom {
		border-top: 1px solid var(--clr-border-2);
	}

	.unassigned__fold-button {
		display: flex;
		/* Align this icon's position with the folded one.
		Prevent any position shifting or jumping. */
		margin-left: -3px;
	}
</style>
