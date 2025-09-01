<script lang="ts">
	import MergeButton from '$components/MergeButton.svelte';
	import PullRequestCard from '$components/PullRequestCard.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { AsyncButton, TestId } from '@gitbutler/ui';

	import type { MergeMethod } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		poll: boolean;
		prNumber: number;
	}

	const { projectId, stackId, branchName, prNumber }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const stackService = inject(STACK_SERVICE);

	// TODO: Make these props so we don't need `!`.
	const repoService = $derived(forge.current.repoService);
	const prService = $derived(forge.current.prService);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branch = $derived(branchResult.current.data);
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchDetails = $derived(branchDetailsResult.current.data);
	const isPushed = $derived(branchDetails?.pushStatus !== 'completelyUnpushed');
	const prResult = $derived(branch?.prNumber ? prService?.get(branch?.prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const parentResult = $derived(stackService.branchParentByName(projectId, stackId, branchName));
	const parent = $derived(parentResult.current.data);
	const hasParent = $derived(!!parent);
	const parentBranchDetailsResult = $derived(
		parent ? stackService.branchDetails(projectId, stackId, parent.name) : undefined
	);
	const parentBranchDetails = $derived(parentBranchDetailsResult?.current.data);
	const parentIsPushed = $derived(parentBranchDetails?.pushStatus !== 'completelyUnpushed');
	const childResult = $derived(stackService.branchChildByName(projectId, stackId, branchName));
	const child = $derived(childResult.current.data);

	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchResponse.current.data);
	const baseBranchRepoResponse = $derived(baseBranchService.repo(projectId));
	const baseBranchRepo = $derived(baseBranchRepoResponse.current.data);
	const repoResult = $derived(repoService?.getInfo());
	const repoInfo = $derived(repoResult?.current.data);

	let shouldUpdateTargetBaseBranch = $derived(
		repoInfo?.deleteBranchAfterMerge === false && !!child?.prNumber
	);
	const baseIsTargetBranch = $derived.by(() => {
		if (forge.current.name === 'gitlab') return true;
		return pr
			? baseBranch?.shortName === pr.baseBranch && baseBranchRepo?.hash === pr.baseRepo?.hash
			: false;
	});

	const prUnit = $derived(prService?.unit.abbr);

	async function handleReopen() {
		if (!pr) return;
		await prService?.reopen(pr.number);
	}

	async function handleMerge(method: MergeMethod) {
		if (!pr) return;
		await prService?.merge(method, pr.number);

		// In a stack, after merging, update the new bottom PR target
		// base branch to master if necessary
		if (baseBranch && shouldUpdateTargetBaseBranch && prService && child?.prNumber) {
			const targetBase = baseBranch.branchName.replace(`${baseBranch.remoteName}/`, '');
			await prService.update(child.prNumber, { targetBase });
		}

		await Promise.all([
			baseBranchService.fetchFromRemotes(projectId),
			baseBranchService.refreshBaseBranch(projectId)
		]);
	}
</script>

<PullRequestCard
	testId={TestId.StackedPullRequestCard}
	{branchName}
	{prNumber}
	{isPushed}
	{hasParent}
	{parentIsPushed}
	{baseIsTargetBranch}
	poll
>
	{#snippet button({ pr, mergeStatus, reopenStatus })}
		{#if pr.state === 'open'}
			<MergeButton
				wide
				{projectId}
				disabled={mergeStatus.disabled}
				tooltip={mergeStatus.tooltip}
				onclick={handleMerge}
			/>
		{:else if !pr.merged}
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
