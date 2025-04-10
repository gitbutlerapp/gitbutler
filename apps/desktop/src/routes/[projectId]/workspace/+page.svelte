<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const projectId = page.params.projectId!;

	const [uiState, stackService] = inject(UiState, StackService);

	const drawer = $derived(uiState.project(projectId).drawerPage);

	const stacksResult = stackService.stacks(projectId);
	const isCommitting = $derived(drawer.current === 'new-commit');
</script>

<ReduxResult {projectId} result={stacksResult.current}>
	{#snippet children(stacks, env)}
		{#if stacks.length > 0}
			{goto(stackPath(env.projectId, stacks[0]!.id))}
		{:else if isCommitting}
			<StackDraft {projectId} />
		{:else}
			<EmptyStatePlaceholder
				image={noBranchesSvg}
				background="none"
				bottomMargin={40}
				topBottomPadding={0}
			>
				{#snippet title()}
					You have no branches
				{/snippet}
				{#snippet caption()}
					Create a new branch for<br />a feature, fix, or idea!
				{/snippet}
			</EmptyStatePlaceholder>
		{/if}
	{/snippet}
</ReduxResult>
