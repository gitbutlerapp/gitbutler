<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import PushButton from './PushButton.svelte';
	import type { CommitType } from './commitList';
	import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';
	import type { GitHubService } from '$lib/github/service';
	import toast from 'svelte-french-toast';

	export let branch: Branch;
	export let type: CommitType;
	export let readonly: boolean;
	export let branchController: BranchController;
	export let githubService: GitHubService;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;

	$: githubServiceState$ = githubService.getState(branch.id);
	$: pr$ = githubService.get(branch.upstreamName);

	let isPushing: boolean;

	async function push() {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		isPushing = false;
	}

	async function createPr(): Promise<PullRequest | undefined> {
		if (githubContext && base?.shortName) {
			return await githubService.createPullRequest(
				githubContext,
				base.shortName,
				branch.name,
				branch.notes,
				branch.id
			);
		} else {
			console.log('Unable to create pull request');
		}
	}
</script>

{#if !readonly && type != 'integrated'}
	<div class="actions">
		{#if githubContext && !$pr$ && type == 'local' && !branch.upstream}
			<PushButton
				wide
				isLoading={isPushing || $githubServiceState$?.busy}
				{projectId}
				{githubContext}
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
		{:else if githubContext && !$pr$ && type == 'remote'}
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
		{/if}
	</div>
{/if}

<style lang="postcss">
	.actions {
		padding-left: var(--space-16);
	}
</style>
