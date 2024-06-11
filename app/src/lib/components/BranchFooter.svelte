<script lang="ts">
	import PassphraseBox from './PassphraseBox.svelte';
	import PushButton, { BranchAction } from './PushButton.svelte';
	import emptyStateImg from '$lib/assets/empty-state/commits-up-to-date.svg?raw';
	import { PromptService } from '$lib/backend/prompt';
	import { project } from '$lib/testing/fixtures';
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

	let isLoading: boolean;

	$: canBePushed = $localCommits.length !== 0 || $unknownCommits.length !== 0;
	$: hasUnknownCommits = $unknownCommits.length > 0;
	$: hasCommits =
		$localCommits.length > 0 || $remoteCommits.length > 0 || $unknownCommits.length > 0;
</script>

{#if !isUnapplied && hasCommits}
	<div class="actions">
		{#if canBePushed}
			{#if $prompt}
				<PassphraseBox prompt={$prompt} error={$promptError} />
			{/if}
			<PushButton
				wide
				projectId={project.id}
				requiresForce={$branch.requiresForce}
				integrate={hasUnknownCommits}
				{isLoading}
				on:trigger={async (e) => {
					isLoading = true;
					try {
						if (e.detail.action === BranchAction.Push) {
							await branchController.pushBranch($branch.id, $branch.requiresForce);
						} else if (e.detail.action === BranchAction.Integrate) {
							await branchController.mergeUpstream($branch.id);
						}
					} catch (e) {
						console.error(e);
					} finally {
						isLoading = false;
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
		padding: 16px;
	}

	/* EMPTY STATE */

	.empty-state {
		display: flex;
		/* justify-content: space-between; */
		align-items: center;
		gap: 20px;
	}

	.empty-state__image {
		flex-shrink: 0;
	}

	.empty-state__text {
		color: var(--clr-text-3);
		flex: 1;
	}
</style>
