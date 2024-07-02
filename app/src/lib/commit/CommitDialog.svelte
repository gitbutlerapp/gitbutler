<script lang="ts">
	import CommitMessageInput from './CommitMessageInput.svelte';
	import { persistedCommitMessage, projectRunCommitHooks } from '$lib/config/config';
	import Button from '$lib/shared/Button.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { slideFade } from '$lib/utils/svelteTransitions';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let projectId: string;
	export let expanded: Writable<boolean>;
	export let hasSectionsAfter: boolean;

	const branchController = getContext(BranchController);
	const selectedOwnership = getContextStore(Ownership);
	const branch = getContextStore(Branch);

	const runCommitHooks = projectRunCommitHooks(projectId);
	const commitMessage = persistedCommitMessage(projectId, $branch.id);

	let isCommitting = false;
	let commitMessageValid = false;
	let isInViewport = false;

	async function commit() {
		const message = $commitMessage;
		isCommitting = true;
		try {
			await branchController.commitBranch(
				$branch.id,
				message.trim(),
				$selectedOwnership.toString(),
				$runCommitHooks
			);
			$commitMessage = '';
		} finally {
			isCommitting = false;
		}
	}
</script>

<div
	class="commit-box"
	class:not-in-viewport={!isInViewport}
	class:no-sections-after={!hasSectionsAfter}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry.isIntersecting) {
				isInViewport = true;
			} else {
				isInViewport = false;
			}
		},
		options: {
			root: null,
			rootMargin: '-1px',
			threshold: 1
		}
	}}
>
	<CommitMessageInput
		bind:commitMessage={$commitMessage}
		bind:valid={commitMessageValid}
		isExpanded={$expanded}
		{commit}
	/>
	<div class="actions" class:commit-box__actions-expanded={$expanded}>
		{#if $expanded && !isCommitting}
			<div class="cancel-btn-wrapper" transition:slideFade={{ duration: 200, axis: 'x' }}>
				<Button
					style="ghost"
					outline
					id="commit-to-branch"
					on:click={() => {
						$expanded = false;
					}}
				>
					Cancel
				</Button>
			</div>
		{/if}
		<Button
			style={$expanded ? 'neutral' : 'ghost'}
			kind="solid"
			outline={!$expanded}
			grow
			loading={isCommitting}
			disabled={(isCommitting || !commitMessageValid || $selectedOwnership.isEmpty()) && $expanded}
			id="commit-to-branch"
			on:click={() => {
				if ($expanded) {
					commit();
				} else {
					$expanded = true;
				}
			}}
		>
			{$expanded ? 'Commit' : 'Start commit'}
		</Button>
	</div>
</div>

<style lang="postcss">
	.commit-box {
		position: sticky;
		bottom: 0;

		display: flex;
		flex-direction: column;
		gap: 12px;

		padding: 14px;
		background: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
		transition: background-color var(--transition-medium);
	}

	.actions {
		display: flex;
		justify-content: right;
		/* gap: 6px; */
	}

	.cancel-btn-wrapper {
		overflow: hidden;
		margin-right: 6px;
	}

	/* MODIFIERS */
	.not-in-viewport {
		z-index: var(--z-ground);
	}

	.no-sections-after {
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}
</style>
