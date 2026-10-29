<script lang="ts">
	import { goto } from "$app/navigation";
	import PRListCard from "$components/branchesPage/PRListCard.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { workspacePath } from "$lib/routes/routes.svelte";
	import { handleApplyOutcome } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";

	import { inject } from "@gitbutler/core/context";

	type Props = {
		projectId: string;
		prNumber: number;
		onerror?: (error: unknown) => void;
	};

	const { projectId, prNumber, onerror }: Props = $props();

	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const prQuery = $derived(prService.get(projectId, prNumber, { forceRefetch: true }));
	const prUnit = $derived(forgeInfo?.unit);

	const stackService = inject(STACK_SERVICE);

	export async function applyPr() {
		const outcome = await stackService.reviewApply({
			projectId,
			reviewId: prNumber,
		});
		handleApplyOutcome(outcome);
		goto(workspacePath(projectId));
	}
</script>

<ReduxResult result={prQuery.result} {projectId} {onerror}>
	{#snippet children(pr)}
		<div class="pr-card">
			<PRListCard
				reviewUnit={prUnit}
				forge={forgeInfo?.name}
				number={pr.number}
				title={pr.title}
				sourceBranch={pr.sourceBranch}
				isDraft={pr.draft ?? false}
				mergedAt={pr.mergedAt}
				closedAt={pr.closedAt}
				noRemote
			/>
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.pr-card {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		pointer-events: none;
	}
</style>
