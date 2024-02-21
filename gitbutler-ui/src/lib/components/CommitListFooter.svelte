<script lang="ts">
	import PushButton, { BranchAction } from './PushButton.svelte';
	import Button from '$lib/components/Button.svelte';
	import { startTransaction } from '@sentry/sveltekit';
	import toast from 'svelte-french-toast';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { PullRequest } from '$lib/github/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch, CommitStatus } from '$lib/vbranches/types';

	export let branch: Branch;
	export let type: CommitStatus;
	export let isUnapplied: boolean;
	export let branchController: BranchController;
	export let branchService: BranchService;
	export let githubService: GitHubService;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;

	$: githubServiceState$ = githubService.getState(branch.id);
	$: githubEnabled$ = githubService.isEnabled$;
	$: pr$ = githubService.get(branch.upstreamName);

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
		if (!githubService.isEnabled()) {
			toast.error('Cannot create PR without GitHub credentials');
			return;
		}

		if (!base?.shortName) {
			toast.error('Cannot create PR without base branch');
			return;
		}

		// Sentry transaction for measuring pr creation latency
		const sentryTxn = startTransaction({ name: 'pull_request_create' });

		isPushing = true;
		try {
			return await branchService.createPr(branch, base.shortName, opts.draft, sentryTxn);
		} finally {
			sentryTxn.finish();
			isPushing = false;
		}
	}
</script>

{#if !isUnapplied && type != 'integrated'}
	<div class="actions">
		{#if $githubEnabled$ && (type == 'local' || type == 'remote')}
			<PushButton
				wide
				isLoading={isPushing || $githubServiceState$?.busy}
				isPr={!!$pr$}
				{type}
				{branch}
				{projectId}
				githubEnabled={$githubEnabled$}
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
	.actions {
		padding-left: var(--space-16);

		&:empty {
			display: none;
		}
	}
</style>
