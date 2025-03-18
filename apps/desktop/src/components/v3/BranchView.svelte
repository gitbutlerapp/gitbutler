<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';

	interface Props {
		stackId: string;
		projectId: string;
		branchName: string;
	}

	const { stackId, projectId, branchName }: Props = $props();

	const [stackService] = inject(StackService);
	const branchState = 'Unpushed'; // TODO: Get this from the branch
</script>

{#if branchName}
	{@const branchResult = stackService.branchByName(projectId, stackId, branchName)}
	<ReduxResult result={branchResult.current}>
		{#snippet children(branch)}
			<div class="branch-view">
				<div class="branch-view__header-container">
					<div class="branch-view__header-title-row">
						<Badge>{branchState}</Badge>
						<h3 class="text-15 text-bold">
							{branch.name}
						</h3>
					</div>

					<div class="text-13 branch-view__header-details-row">Contributors: ...</div>
				</div>

				<div class="branch-view__review-card-container">Review card goes here</div>

				<ChangedFiles type="branch" {projectId} {stackId} {branchName} />
			</div>
		{/snippet}
	</ReduxResult>
{/if}

<style>
	.branch-view {
		display: flex;
		padding: 14px;
		flex-direction: column;
		gap: 16px;
		align-self: stretch;
		height: 100%;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.branch-view__header-container {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
	}

	.branch-view__header-title-row {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.branch-view__review-card-container {
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		padding: 14px;
	}
</style>
