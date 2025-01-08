<script lang="ts">
	import CommitMessageInput from './CommitMessageInput.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { cloudCommunicationFunctionality } from '$lib/config/uiFeatureFlags';
	import { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { BranchController } from '$lib/vbranches/branchController';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { BranchStack } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { slideFade } from '@gitbutler/ui/utils/transitions';
	import { tick } from 'svelte';
	import type { Writable } from 'svelte/store';

	interface Props {
		projectId: string;
		expanded: Writable<boolean>;
		hasSectionsAfter: boolean;
	}

	const { projectId, expanded, hasSectionsAfter }: Props = $props();

	const branchController = getContext(BranchController);
	const syncedSnapshotService = getContext(SyncedSnapshotService);
	const canTakeSnapshot = syncedSnapshotService.canTakeSnapshot;
	const selectedOwnership = getContextStore(SelectedOwnership);
	const stack = getContextStore(BranchStack);
	const commitMessage = persistedCommitMessage(projectId, $stack.id);

	let commitMessageInput = $state<CommitMessageInput>();
	let isCommitting = $state(false);
	let commitMessageValid = $state(false);
	let isInViewport = $state(false);

	async function commit() {
		const message = $commitMessage;
		isCommitting = true;
		try {
			await branchController.commitBranch($stack.id, message.trim(), $selectedOwnership.toString());
			$commitMessage = '';

			if (commitAndPublish) {
				syncedSnapshotService.takeSyncedSnapshot($stack.id);
			}
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
		commitMessageInput?.focus();
	}

	const canShowCommitAndPublish = $derived($cloudCommunicationFunctionality && $canTakeSnapshot);

	let commitAndPublish = $state(false);
	let commitButton = $state<DropDownButton>();
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
		{#if $expanded && canShowCommitAndPublish}
			<DropDownButton
				bind:this={commitButton}
				onclick={() => {
					if ($expanded) {
						commit();
					} else {
						focus();
					}
				}}
				style="pop"
				kind="solid"
				grow
				outline={!$expanded}
				loading={isCommitting}
				disabled={(isCommitting || !commitMessageValid || $selectedOwnership.nothingSelected()) &&
					$expanded}
			>
				{commitAndPublish ? 'Commit and bleep' : 'Commit'}

				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Commit and bleep"
							onclick={() => {
								commitAndPublish = true;
								commitButton?.close();
							}}
						/>

						<ContextMenuItem
							label="Commit"
							onclick={() => {
								commitAndPublish = false;
								commitButton?.close();
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>
		{:else}
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
		{/if}
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
