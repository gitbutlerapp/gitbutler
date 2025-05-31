<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import WorktreeChangesSelectAll from '$components/v3/WorktreeChangesSelectAll.svelte';
	import { assignedChangesFocusableId } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { AssignmentDropHandler } from '$lib/hunks/dropHandler';
	import { AssignmentService } from '$lib/selection/assignmentService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();

	const [stackService, diffService, uiState, assignmentService] = inject(
		StackService,
		DiffService,
		UiState,
		AssignmentService
	);
	const stackState = $derived(uiState.stack(projectId));
	const projectState = $derived(uiState.project(projectId));
	const sourceStackId = $derived(projectState.commitSourceId.current);
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit' && stackId === sourceStackId);

	const assignmentDZHandler = new AssignmentDropHandler(
		projectId,
		diffService,
		assignmentService,
		stackId
	);

	const defaultBranchResult = $derived(stackService.defaultBranch(projectId, stackId));
	const defaultBranchName = $derived(defaultBranchResult?.current.data);

	function startCommit() {
		assignmentService.checkAll(stackId);
		projectState.drawerPage.set('new-commit');
		projectState.commitSourceId.set(stackId); // Unassigned changes.
		projectState.stackId.set(stackId);
		if (defaultBranchName) {
			stackState.selection.set({ branchName: defaultBranchName });
		}
	}
</script>

<Dropzone handlers={[assignmentDZHandler].filter(isDefined)}>
	{#snippet overlay({ hovered, activated })}
		<CardOverlay {hovered} {activated} />
	{/snippet}
	<div class="assigned-changes" use:focusable={{ id: assignedChangesFocusableId(stackId) }}>
		<div class="assigned-changes__title text-14 text-bold">
			{#if isCommitting}
				<WorktreeChangesSelectAll {stackId} />
			{/if}
			assigned changes
		</div>
		<Button
			kind={isCommitting ? 'outline' : 'solid'}
			type="button"
			size="cta"
			wide
			disabled={isCommitting}
			onclick={startCommit}
		>
			Start a commit…
		</Button>
	</div>
</Dropzone>

<style lang="postcss">
	.assigned-changes {
		margin: 12px 12px 0;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.assigned-changes__title {
		display: flex;
		padding: 8px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
