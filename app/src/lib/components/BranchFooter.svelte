<script lang="ts">
	import PassphraseBox from './PassphraseBox.svelte';
	import PushButton, { BranchAction } from './PushButton.svelte';
	import emptyStateImg from '$lib/assets/empty-state/commits-up-to-date.svg?raw';
	import { PromptService } from '$lib/backend/prompt';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getLocalCommits, getRemoteCommits, getUnknownCommits } from '$lib/vbranches/contexts';
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
	const unknownCommits = getUnknownCommits();

	$: hasCommits =
		$localCommits.length > 0 || $remoteCommits.length > 0 || $unknownCommits.length > 0;

	let isLoading: boolean;
	$: isPushed = $localCommits.length === 0 && $unknownCommits.length === 0;
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
						if (e.detail.action === BranchAction.Push) {
							isLoading = true;
							await branchController.pushBranch($branch.id, $branch.requiresForce);
							isLoading = false;
						} else if (e.detail.action === BranchAction.Rebase) {
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
			<div class="empty-state">
				<span class="text-base-body-12 empty-state__text"
					>Your branch is up to date with the remote.</span
				>

				<i class="empty-state__image">
					{@html emptyStateImg}
				</i>
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.actions {
		background: var(--clr-bg-1);
		padding: var(--size-16);
	}

	/* EMPTY STATE */

	.empty-state {
		display: flex;
		/* justify-content: space-between; */
		align-items: center;
		gap: var(--size-20);
	}

	.empty-state__image {
		flex-shrink: 0;
	}

	.empty-state__text {
		color: var(--clr-text-3);
		flex: 1;
		/* max-width: 8rem; */
	}
</style>
