<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import PushButton from './PushButton.svelte';
	import type { CommitType } from './commitList';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import type { PrService } from '$lib/github/pullrequest';
	import { sleep } from '$lib/utils/sleep';
	import toast from 'svelte-french-toast';

	export let branch: Branch;
	export let type: CommitType;
	export let readonly: boolean;
	export let branchController: BranchController;
	export let prService: PrService;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;

	$: prServiceState$ = prService.getState(branch.id);
	$: pr$ = prService.get(branch.shortName);

	let isPushing: boolean;

	async function push(opts?: { createPr: boolean }) {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		if (opts?.createPr) {
			await sleep(500); // Needed by GitHub
			await createPr();
		}
		isPushing = false;
	}

	async function createPr(): Promise<void> {
		if (githubContext && base?.branchName && branch.shortName) {
			const pr = await prService.createPullRequest(
				githubContext,
				branch.shortName,
				base.shortName,
				branch.name,
				branch.notes,
				branch.id
			);
			if (pr) {
				await prService.reload();
			}
			return;
		} else {
			console.log('Unable to create pull request');
		}
	}
</script>

{#if !readonly && type != 'integrated'}
	<div class="actions">
		{#if githubContext && !$pr$ && type == 'local'}
			<PushButton
				wide
				isLoading={isPushing || $prServiceState$?.busy}
				{projectId}
				{githubContext}
				on:trigger={async (e) => {
					try {
						await push({ createPr: e.detail.with_pr });
					} catch {
						toast.error('Failed to create pull qequest');
					}
				}}
			/>
		{:else if githubContext && !$pr$ && type == 'remote'}
			<Button
				wide
				kind="outlined"
				color="primary"
				loading={isPushing || $prServiceState$?.busy}
				on:click={async () => {
					try {
						await push({ createPr: true });
					} catch (e) {
						toast.error('Failed to create pull qequest');
					}
				}}
			>
				Create Pull Request
			</Button>
		{:else if type == 'local'}
			<Button
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
		{/if}
	</div>
{/if}

<style lang="postcss">
	.actions {
		padding-left: var(--space-16);
	}
</style>
