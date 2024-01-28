<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch, CommitStatus } from '$lib/vbranches/types';
	import PushButton from './PushButton.svelte';
	import type { PullRequest } from '$lib/github/types';
	import type { GitHubService } from '$lib/github/service';
	import toast from 'svelte-french-toast';
	import { startTransaction } from '@sentry/sveltekit';
	import type { BranchService } from '$lib/branches/service';

	export let branch: Branch;
	export let type: CommitStatus;
	export let readonly: boolean;
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

	async function push() {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		isPushing = false;
	}

	async function createPr(): Promise<PullRequest | undefined> {
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
			return await branchService.createPr(branch, base.shortName, sentryTxn);
		} catch (err) {
			toast.error(err as string);
			console.error(err);
		} finally {
			sentryTxn.finish();
			isPushing = false;
		}
	}

	$: {
		console.log('branch type', type);
	}
</script>

{#if !readonly && type != 'integrated'}
	<div class="actions">
		{#if $githubEnabled$ && !$pr$ && type == 'local' && !branch.upstream}
			<PushButton
				wide
				isLoading={isPushing || $githubServiceState$?.busy}
				{projectId}
				githubEnabled={$githubEnabled$}
				on:trigger={async (e) => {
					try {
						if (e.detail.with_pr) {
							await createPr();
						} else {
							await push();
						}
					} catch {
						toast.error('Failed to create pull request');
					}
				}}
			/>
		{:else if $githubEnabled$ && !$pr$ && type == 'remote'}
			<Button
				wide
				kind="outlined"
				color="primary"
				loading={isPushing || $githubServiceState$?.busy}
				on:click={async () => {
					try {
						await createPr();
					} catch (e) {
						toast.error('Failed to create pull request');
					}
				}}
			>
				Create Pull Request
			</Button>
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
