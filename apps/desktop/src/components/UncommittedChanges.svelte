<script lang="ts">
	import BranchFiles from '$components/BranchFiles.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import CommitDialog from '$components/CommitDialog.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import InfoMessage from '$components/InfoMessage.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { BranchFileDzHandler, BranchHunkDzHandler } from '$lib/branches/dropHandler';
	import { Project } from '$lib/project/project';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	interface Props {
		commitBoxOpen: Writable<boolean>;
	}

	const { commitBoxOpen }: Props = $props();

	const project = getContext(Project);
	const branchStore = getContextStore(BranchStack);
	const stackService = getContext(StackService);

	const stack = $derived($branchStore);

	let commitDialog = $state<ReturnType<typeof CommitDialog>>();
	const dzFileHandler = $derived(
		new BranchFileDzHandler(stackService, project.id, stack.id, stack.ownership)
	);
	const dzHunkHandler = $derived(new BranchHunkDzHandler(stackService, project.id, stack));
</script>

<div class="branch-card__files">
	<Dropzone handlers={[dzHunkHandler, dzFileHandler]}>
		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
		<BranchFiles
			projectId={project.id}
			isUnapplied={false}
			files={stack.files}
			branches={stack.validSeries}
			showCheckboxes={$commitBoxOpen}
			allowMultiple
			commitDialogExpanded={commitBoxOpen}
			focusCommitDialog={() => commitDialog?.focus()}
		/>
		{#if stack.conflicted}
			<div class="card-notifications">
				<InfoMessage filled outlined={false} style="error">
					{#snippet title()}
						{#if stack.files.some((f) => f.conflicted)}
							This virtual branch conflicts with upstream changes. Please resolve all conflicts and
							commit before you can continue.
						{:else}
							Please commit your resolved conflicts to continue.
						{/if}
					{/snippet}
				</InfoMessage>
			</div>
		{/if}
	</Dropzone>

	<CommitDialog
		bind:this={commitDialog}
		projectId={project.id}
		expanded={commitBoxOpen}
		hasSectionsAfter={stack.validSeries.flatMap((s) => s.patches).length > 0}
	/>
</div>

<style>
	.branch-card__files {
		border-radius: 0 0 var(--radius-m) var(--radius-m) !important;
		border: 1px solid var(--clr-border-2);
		border-top-width: 0;
		background: var(--clr-bg-1);

		display: flex;
		flex-direction: column;
		flex: 1;
		height: 100%;
	}

	.card-notifications {
		display: flex;
		flex-direction: column;
		padding: 12px;
	}
</style>
