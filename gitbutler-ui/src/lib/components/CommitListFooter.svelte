<script lang="ts">
	import PushButton, { BranchAction } from './PushButton.svelte';
	import Button from '$lib/components/Button.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContextByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import toast from 'svelte-french-toast';
	import type { BranchService } from '$lib/branches/service';
	import type { PullRequest } from '$lib/github/types';
	import type { BaseBranch, Branch, CommitStatus } from '$lib/vbranches/types';

	export let branch: Branch;
	export let type: CommitStatus;
	export let isUnapplied: boolean;
	export let branchService: BranchService;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;
	export let hasCommits: boolean;

	const githubService = getContextByClass(GitHubService);
	const branchController = getContextByClass(BranchController);

	$: githubServiceState$ = githubService.getState(branch.id);
	$: pr = githubService.getPr(branch.upstreamName);

	let isPushing: boolean;
	let isMerging: boolean;

	interface CreatePrOpts {
		draft: boolean;
	}

	const defaultPrOpts: CreatePrOpts = {
		draft: true
	};

	async function push() {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		isPushing = false;
	}

	async function createPr(createPrOpts: CreatePrOpts): Promise<PullRequest | undefined> {
		const opts = { ...defaultPrOpts, ...createPrOpts };
		if (!githubService.isEnabled) {
			toast.error('Cannot create PR without GitHub credentials');
			return;
		}

		if (!base?.shortName) {
			toast.error('Cannot create PR without base branch');
			return;
		}

		isPushing = true;
		try {
			return await branchService.createPr(branch, base.shortName, opts.draft);
		} finally {
			isPushing = false;
		}
	}
</script>

{#if !isUnapplied && type != 'integrated'}
	<div class="actions" class:hasCommits>
		{#if githubService.isEnabled && (type == 'local' || type == 'remote')}
			<PushButton
				wide
				isLoading={isPushing || $githubServiceState$?.busy}
				isPr={!!pr}
				{type}
				{branch}
				{projectId}
				githubEnabled={true}
				on:trigger={async (e) => {
					try {
						if (e.detail.action == BranchAction.Pr) {
							await createPr({ draft: false });
						} else if (e.detail.action == BranchAction.DraftPr) {
							await createPr({ draft: true });
						} else {
							await push();
						}
					} catch (e) {
						console.error(e);
					}
				}}
			/>
		{:else if type == 'local'}
			<Button
				wide
				kind="outlined"
				color="primary"
				loading={isPushing}
				on:click={async () => {
					try {
						await push();
					} catch {
						toast.error('Failed to push');
					}
				}}
			>
				{#if branch.requiresForce}
					Force Push
				{:else}
					Push
				{/if}
			</Button>
		{:else if type == 'upstream'}
			<Button
				wide
				color="warn"
				loading={isMerging}
				on:click={async () => {
					isMerging = true;
					try {
						await branchController.mergeUpstream(branch.id);
					} catch (err) {
						toast.error('Failed to merge upstream commits');
					} finally {
						isMerging = false;
					}
				}}
			>
				Merge upstream commits
			</Button>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.hasCommits {
		padding-left: var(--space-16);
	}
	.actions {
		&:empty {
			display: none;
		}
	}
</style>
