<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch, CommitStatus } from '$lib/vbranches/types';
	import PushButton from './PushButton.svelte';
	import type { PullRequest } from '$lib/github/types';
	import type { GitHubService } from '$lib/github/service';
	import toast from 'svelte-french-toast';
	import { startTransaction } from '@sentry/sveltekit';
	import { sleep } from '$lib/utils/sleep';

	export let branch: Branch;
	export let type: CommitStatus;
	export let readonly: boolean;
	export let branchController: BranchController;
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
		isPushing = true;
		const txn = startTransaction({ name: 'pull_request_create' });

		try {
			if (!githubService.isEnabled()) {
				toast.error('Cannot create PR without GitHub credentials');
				return;
			}

			if (!base?.shortName) {
				toast.error('Cannot create PR without base branch');
				return;
			}

			// Push if local commits
			if (branch.commits.some((c) => !c.isRemote)) {
				const pushBranchSpan = txn.startChild({ op: 'branch_push' });
				await branchController.pushBranch(branch.id, branch.requiresForce);
				pushBranchSpan.finish();
			}

			let waitRetries = 0;
			while (!branch.upstreamName) {
				console.log('waiting for branch name');
				await sleep(200);
				if (waitRetries++ > 100) break;
			}

			if (!branch.upstreamName) {
				toast.error('Cannot create PR without remote branch name');
				return;
			}

			const createPrSpan = txn.startChild({ op: 'pr_api_create' });
			const resp = await githubService.createPullRequest(
				base.shortName,
				branch.name,
				branch.notes,
				branch.id,
				branch.upstreamName
			);
			createPrSpan.finish();
			return resp;
		} catch (e) {
			console.error('Failed to create PR', e);
			toast.error('Failed to create pull request');
		} finally {
			isPushing = false;
			txn.finish();
		}
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
	}
</style>
