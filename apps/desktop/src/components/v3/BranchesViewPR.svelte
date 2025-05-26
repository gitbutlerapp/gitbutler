<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { pushStatusToColor } from '$lib/stacks/stack';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	type Props = {
		projectId: string;
		prNumber: number;
		onerror?: (error: unknown) => void;
	};

	const { projectId, prNumber, onerror }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);
	const prResult = $derived(prService?.get(prNumber, { forceRefetch: true }));

	const uiState = getContext(UiState);
	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);

	const selected = $derived(branchesState.current.prNumber === prNumber);
</script>

<ReduxResult result={prResult?.current} {projectId} {onerror}>
	{#snippet children(pr, { projectId })}
		<BranchCard
			type="pr-branch"
			{selected}
			branchName={pr.sourceBranch}
			{projectId}
			readonly
			active
			trackingBranch={pr.sourceBranch}
			lastUpdatedAt={new Date(pr.updatedAt).getTime()}
			lineColor={getColorFromBranchType(pushStatusToColor('nothingToPush'))}
		>
			{#snippet branchContent()}
				<div class="text-13 pr-branch-content">
					<h2 class="text-14 text-semibold">
						{pr.title}
					</h2>
					<Markdown content={pr.body} />
				</div>
			{/snippet}
		</BranchCard>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.pr-branch-content {
		padding: 8px;
		color: var(--clr-text-1);
		line-height: 1.5;
	}
</style>
