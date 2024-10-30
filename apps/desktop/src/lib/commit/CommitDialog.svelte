<script lang="ts">
	import CommitMessageInput from './CommitMessageInput.svelte';
	import { persistedCommitMessage, projectRunCommitHooks } from '$lib/config/config';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { BranchController } from '$lib/vbranches/branchController';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { slideFade } from '@gitbutler/ui/utils/transitions';
	import { tick } from 'svelte';
	import type { Writable } from 'svelte/store';

	export let projectId: string;
	export let expanded: Writable<boolean>;
	export let hasSectionsAfter: boolean;

	const branchController = getContext(BranchController);
	const selectedOwnership = getContextStore(SelectedOwnership);
	const branch = getContextStore(VirtualBranch);

	const runCommitHooks = projectRunCommitHooks(projectId);
	const commitMessage = persistedCommitMessage(projectId, $branch.id);

	let commitMessageInput: CommitMessageInput;
	let isCommitting = false;
	let commitMessageValid = false;
	let isInViewport = false;

	async function commit() {
		const message = $commitMessage;
		isCommitting = true;
		try {
			await branchController.commitBranch(
				$branch.id,
				$branch.name,
				message.trim(),
				$selectedOwnership.toString(),
				$runCommitHooks
			);
			$commitMessage = '';
		} finally {
			isCommitting = false;
		}
	}

	function close() {
		$expanded = false;
	}

	export async function focus() {
		$expanded = true;
		await tick();
		commitMessageInput.focus();
	}
</script>

<div
	class="commit-box"
	class:not-in-viewport={!isInViewport}
	class:no-sections-after={!hasSectionsAfter}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry?.isIntersecting) {
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
		bind:this={commitMessageInput}
		bind:commitMessage={$commitMessage}
		bind:valid={commitMessageValid}
		isExpanded={$expanded}
		cancel={close}
		{commit}
	/>
	<div class="actions" class:commit-box__actions-expanded={$expanded}>
		{#if $expanded && !isCommitting}
			<div class="cancel-btn-wrapper" transition:slideFade={{ duration: 200, axis: 'x' }}>
				<Button style="ghost" outline id="commit-to-branch" onclick={close}>Cancel</Button>
			</div>
		{/if}
		<Button
			style="pop"
			kind="solid"
			outline={!$expanded}
			grow
			loading={isCommitting}
			disabled={(isCommitting || !commitMessageValid || $selectedOwnership.nothingSelected()) &&
				$expanded}
			id="commit-to-branch"
			onclick={() => {
				if ($expanded) {
					commit();
				} else {
					focus();
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
		border-radius: 0 0 var(--radius-m) var(--radius-m) !important;
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
