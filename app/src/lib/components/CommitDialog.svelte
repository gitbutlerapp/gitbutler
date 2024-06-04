<script lang="ts">
	import Button from './Button.svelte';
	import CommitMessageInput from '$lib/components/CommitMessageInput.svelte';
	import { projectRunCommitHooks, persistedCommitMessage } from '$lib/config/config';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch } from '$lib/vbranches/types';
	import { quintOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import type { Writable } from 'svelte/store';

	export let projectId: string;
	export let expanded: Writable<boolean>;

	const branchController = getContext(BranchController);
	const selectedOwnership = getContextStore(Ownership);
	const branch = getContextStore(Branch);

	const runCommitHooks = projectRunCommitHooks(projectId);
	const commitMessage = persistedCommitMessage(projectId, $branch.id);

	let isCommitting = false;

	let commitMessageValid = false;

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

<div class="commit-box" class:commit-box__expanded={$expanded}>
	{#if $expanded}
		<div class="commit-box__expander" transition:slide={{ duration: 150, easing: quintOut }}>
			<CommitMessageInput
				bind:commitMessage={$commitMessage}
				bind:valid={commitMessageValid}
				{commit}
			/>
		</div>
	{/if}
	<div class="actions">
		{#if $expanded && !isCommitting}
			<Button
				style="ghost"
				kind="solid"
				id="commit-to-branch"
				on:click={() => {
					$expanded = false;
				}}
			>
				Cancel
			</Button>
		{/if}
		<Button
			style="neutral"
			kind="solid"
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
		display: flex;
		flex-direction: column;
		padding: 14px;
		background: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
		transition: background-color var(--transition-medium);
	}

	.commit-box__expander {
		display: flex;
		flex-direction: column;
		margin-bottom: 12px;
	}

	.actions {
		display: flex;
		justify-content: right;
		gap: 6px;
	}

	.commit-box__expanded {
		background-color: var(--clr-bg-2);
	}
</style>
