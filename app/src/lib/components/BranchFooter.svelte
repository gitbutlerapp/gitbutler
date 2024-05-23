<script lang="ts">
	import PassphraseBox from './PassphraseBox.svelte';
	import PushButton, { BranchAction } from './PushButton.svelte';
	import { PromptService } from '$lib/backend/prompt';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getLocalCommits, getRemoteCommits, getUpstreamCommits } from '$lib/vbranches/contexts';
	import { Branch } from '$lib/vbranches/types';

	export let isUnapplied: boolean;

	const branchController = getContext(BranchController);
	const promptService = getContext(PromptService);
	const branch = getContextStore(Branch);

	const [prompt, promptError] = promptService.reactToPrompt({
		branchId: $branch.id,
		timeoutMs: 30000
	});

	const localCommits = getLocalCommits();
	const remoteCommits = getRemoteCommits();
	const unknownCommits = getUpstreamCommits();

	$: hasCommits =
		$localCommits.length > 0 || $remoteCommits.length > 0 || $unknownCommits.length > 0;

	let isLoading: boolean;
	$: isPushed = $localCommits.length == 0 && $unknownCommits.length == 0;
</script>

{#if !isUnapplied && hasCommits}
	<div class="actions">
		{#if !isPushed}
			{#if $prompt}
				<PassphraseBox prompt={$prompt} error={$promptError} />
			{/if}
			<PushButton
				wide
				branch={$branch}
				{isLoading}
				on:trigger={async (e) => {
					try {
						if (e.detail.action == BranchAction.Push) {
							isLoading = true;
							await branchController.pushBranch($branch.id, $branch.requiresForce);
							isLoading = false;
						} else if (e.detail.action == BranchAction.Rebase) {
							isLoading = true;
							await branchController.mergeUpstream($branch.id);
							isLoading = false;
						}
					} catch (e) {
						console.error(e);
					}
				}}
			/>
		{:else}
			<span class="text-base-body-11 text-in-the-bottom">
				Branch {$branch.name} is up to date with the remote.
			</span>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.text-in-the-bottom {
		color: var(--clr-scale-ntrl-50);
	}
	.actions {
		background: var(--clr-bg-1);
		padding: var(--size-16);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		border: 1px solid var(--clr-border-2);
	}
</style>
