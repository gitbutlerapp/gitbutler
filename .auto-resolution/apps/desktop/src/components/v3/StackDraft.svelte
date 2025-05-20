<script lang="ts">
	import BranchCard from '$components/v3/BranchCard.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
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

<div data-testid={TestId.StackDraft} class="stack-draft">
	<BranchCard
		type="draft-branch"
		{projectId}
		{branchName}
		iconName="branch-local"
		readonly={false}
		lineColor="var(--clr-commit-local)"
	/>
	<CommitGoesHere selected draft />
</div>

<style lang="postcss">
	.stack-draft {
		padding: 12px;
		border-right: 1px solid var(--clr-border-2);
	}
</style>
