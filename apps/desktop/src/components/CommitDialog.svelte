<script lang="ts">
	import CommitMessageInput from '$components/CommitMessageInput.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import DropDownButton from '$components/DropDownButton.svelte';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { BranchStack } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { SelectedOwnership } from '$lib/branches/ownership';
	import { persistedCommitMessage, projectRunCommitHooks } from '$lib/config/config';
	import { cloudCommunicationFunctionality } from '$lib/config/uiFeatureFlags';
	import { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
	import { HooksService } from '$lib/hooks/hooksService';
	import { showError } from '$lib/notifications/toasts';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import * as toasts from '$lib/utils/toasts';
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
	const hooksService = getContext(HooksService);
	const posthog = getContext(PostHogWrapper);
	const syncedSnapshotService = getContext(SyncedSnapshotService);
	const canTakeSnapshot = syncedSnapshotService.canTakeSnapshot;
	const selectedOwnership = getContextStore(SelectedOwnership);
	const stack = getContextStore(BranchStack);
	const commitMessage = persistedCommitMessage(projectId, $stack.id);
	const canShowCommitAndPublish = $derived($cloudCommunicationFunctionality && $canTakeSnapshot);
	const runHooks = projectRunCommitHooks(projectId);

	let commitMessageInput = $state<CommitMessageInput>();
	let isCommitting = $state(false);
	let commitMessageValid = $state(false);
	let isInViewport = $state(false);

	let commitAndPublish = $state(false);
	let commitButton = $state<DropDownButton>();

	async function commit() {
		isCommitting = true;
		const message = $commitMessage;
		const ownership = $selectedOwnership.toString();

		try {
			if ($runHooks) {
				const preCommitHook = await hooksService.preCommit(projectId, ownership);

				if (preCommitHook.status === 'success') {
					toasts.success('Pre-commit hook successful');
				} else if (preCommitHook.status === 'failure') {
					showError('Pre-commit hook failed', preCommitHook.error);
					return; // Abort commit if hook failed.
				}
			}
			await branchController.commit($stack.id, message.trim(), ownership);
		} catch (err: unknown) {
			showError('Failed to commit changes', err);
			posthog.capture('Commit Failed', { error: err });
			return;
		} finally {
			isCommitting = false;
		}

		// Run both without awaiting unless commit failed.
		runPostCommitActions();
		if ($runHooks) {
			runPostCommitHook();
		}
	}

	async function runPostCommitActions() {
		// Clear the commit message editor.
		commitMessage.set('');

		// Publishing a snapshot seems to imply posting a bleep.
		if (commitAndPublish) {
			await syncedSnapshotService.takeSyncedSnapshot($stack.id);
		}
	}

	async function runPostCommitHook() {
		const postCommitHook = await hooksService.postCommit(projectId);
		if (postCommitHook.status === 'failure') {
			showError('Post-commit hook failed', postCommitHook.error);
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
				<Button style="neutral" id="commit-to-branch" onclick={close}>Cancel</Button>
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
				grow
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
