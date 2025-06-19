<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [uiState, stackService] = inject(UiState, StackService);
	const draftBranchName = $derived(uiState.global.draftBranchName);

	// Automatic branch name suggested by back end.
	let newName = $state('');
	let draftPanelEl: HTMLDivElement | undefined = $state();

	const newNameResult = stackService.newBranchName(projectId);
	newNameResult.then((name) => {
		newName = name.data || '';
	});

	$effect(() => {
		if (newName && !draftBranchName.current) {
			draftBranchName.set(newName);
		}
	});

	const branchName = $derived(draftBranchName.current || newName);
</script>

<div
	bind:this={draftPanelEl}
	data-testid={TestId.StackDraft}
	class="draft-stack"
	style:width={uiState.global.stackWidth.current + 'rem'}
>
	<div class="new-commit-view" data-testid={TestId.NewCommitView}>
		<NewCommitView {projectId} />
	</div>
	<BranchCard
		type="draft-branch"
		{projectId}
		{branchName}
		readonly={false}
		lineColor="var(--clr-commit-local)"
	>
		{#snippet branchContent()}
			<CommitGoesHere selected last />
		{/snippet}
	</BranchCard>
	<Resizer
		persistId="resizer-darft-panel"
		viewport={draftPanelEl}
		direction="right"
		minWidth={16}
		maxWidth={64}
		syncName="panel1"
		dblclickSize
	/>
</div>

<style lang="postcss">
	.draft-stack {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		padding: 12px;
		border-right: 1px solid var(--clr-border-2);
	}
	.new-commit-view {
		margin-bottom: 12px;
		padding: 12px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}
</style>
