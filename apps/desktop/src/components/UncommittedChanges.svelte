<script lang="ts">
	import BranchFiles from '$components/BranchFiles.svelte';
	import CommitDialog from '$components/CommitDialog.svelte';
	import Dropzones from '$components/Dropzones.svelte';
	import InfoMessage from '$components/InfoMessage.svelte';
	import { Project } from '$lib/project/project';
	import { BranchStack } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	interface Props {
		commitBoxOpen: Writable<boolean>;
	}

	const { commitBoxOpen }: Props = $props();

	const project = getContext(Project);
	const branchStore = getContextStore(BranchStack);

	const stack = $derived($branchStore);

	let commitDialog = $state<ReturnType<typeof CommitDialog>>();
</script>

<div class="branch-card__files">
	<Dropzones type="file">
		<BranchFiles
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
	</Dropzones>

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
