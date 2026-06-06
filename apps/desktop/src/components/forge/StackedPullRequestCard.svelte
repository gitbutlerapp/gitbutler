<script lang="ts">
	import MergeButton from "$components/forge/MergeButton.svelte";
	import PullRequestCard from "$components/forge/PullRequestCard.svelte";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { REPO_SERVICE } from "$lib/forge/repoService.svelte";
	import { pullRequestTargetsBaseBranch } from "$lib/forge/shared/pullRequestTargets";
	import { inject } from "@gitbutler/core/context";
	import { AsyncButton, TestId } from "@gitbutler/ui";

	import type { MergeMethod } from "$lib/forge/interface/types";
	import type { Segment } from "@gitbutler/but-sdk";

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		parent: Segment | undefined;
		child: Segment | undefined;
		isPushed: boolean;
		poll: boolean;
		prNumber: number;
	}

	const { projectId, branchName, parent, child, isPushed, poll, prNumber }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const prService = inject(PR_SERVICE);
	const repoService = inject(REPO_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);

	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const repoInfoEnabled = $derived(!!forgeInfo?.capabilities.repoInfo);

	const prQuery = $derived(prNumber ? prService.get(projectId, prNumber) : undefined);
	const pr = $derived(prQuery?.response);

	const hasParent = $derived(!!parent);
	const parentIsPushed = $derived(parent?.pushStatus !== "completelyUnpushed");
	const childPrNumber = $derived(child?.metadata?.review.pullRequest);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const baseRepoQuery = $derived(baseBranchService.repo(projectId));
	const baseRepo = $derived(baseRepoQuery.response);
	const repoQuery = $derived(repoInfoEnabled ? repoService.getInfo(projectId) : undefined);
	const repoInfo = $derived(repoQuery?.response);

	const prBaseRepoUrlQuery = $derived(
		prNumber ? prService.getBaseRepoUrl(projectId, prNumber) : undefined,
	);
	const prBaseRepoUrl = $derived(prBaseRepoUrlQuery?.response);

	let shouldUpdateTargetBaseBranch = $derived(
		repoInfo?.deleteBranchAfterMerge === false && !!childPrNumber,
	);
	const baseIsTargetBranch = $derived(
		pullRequestTargetsBaseBranch({
			pr,
			baseBranchShortName: baseBranch?.shortName,
			baseBranchRepoHash: baseRepo?.hash,
			prBaseRepoUrl,
			forgeName: forgeInfo?.name ?? "default",
		}),
	);

	const prUnit = $derived(forgeInfo?.unit.abbr);

	async function handleReopen() {
		if (!pr) return;
		await prService.reopen(projectId, pr.number);
	}

	async function handleMerge(method: MergeMethod) {
		if (!pr) return;
		await prService.merge(projectId, method, pr.number);

		// In a stack, after merging, update the new bottom PR target
		// base branch to master if necessary
		if (baseBranch && shouldUpdateTargetBaseBranch && childPrNumber) {
			const targetBase = baseBranch.branchName.replace(`${baseBranch.remoteName}/`, "");
			await prService.update(projectId, childPrNumber, { targetBase });
		}

		await Promise.all([
			baseBranchService.fetchFromRemotes(projectId),
			baseBranchService.refreshBaseBranch(projectId),
		]);
	}
</script>

<PullRequestCard
	{projectId}
	testId={TestId.StackedPullRequestCard}
	{branchName}
	{prNumber}
	{isPushed}
	{hasParent}
	{parentIsPushed}
	{baseIsTargetBranch}
	{poll}
>
	{#snippet button({ pr, mergeStatus, reopenStatus, setDraft })}
		{#if !pr.closedAt && !pr.mergedAt}
			{#if pr.draft}
				<AsyncButton wide kind="outline" action={() => setDraft(false)}
					>Ready for review</AsyncButton
				>
			{:else}
				<MergeButton
					wide
					{projectId}
					disabled={mergeStatus.disabled}
					tooltip={mergeStatus.tooltip}
					isDraft={pr.draft ?? false}
					onSetDraft={setDraft}
					onclick={handleMerge}
				/>
			{/if}
		{:else if !pr.mergedAt}
			<AsyncButton
				kind="outline"
				disabled={reopenStatus.disabled}
				tooltip={reopenStatus.tooltip}
				action={handleReopen}
			>
				{`Reopen ${prUnit}`}
			</AsyncButton>
		{/if}
	{/snippet}
</PullRequestCard>

<style lang="postcss">
</style>
