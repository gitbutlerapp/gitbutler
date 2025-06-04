<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import DataContextMenu from '$components/v3/DataContextMenu.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import ActionService from '$lib/actions/actionService.svelte';
	import { snapshotChangesFocusableId } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { OplogService } from '$lib/history/oplogService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import { getContext as svelteGetContext, untrack } from 'svelte';
	import type { SelectionId } from '$lib/selection/key';
	import type { Writable } from 'svelte/store';

	type Props = {
		projectId: string;
		before: string;
		after: string;
		timestamp: string;
		selectionId: SelectionId;
	};

	const { projectId, before, after, timestamp, selectionId }: Props = $props();

	const oplogService = getContext(OplogService);
	const actionService = getContext(ActionService);
	const [revertSnapshot] = actionService.revertSnapshot;

	const focusableId = $derived(snapshotChangesFocusableId(before, after));
	const focusableIds = svelteGetContext<Writable<string[]>>('snapshot-focusables');

	async function restore(id: string, description: string) {
		await revertSnapshot({ projectId, snapshot: id, description });
		// In some cases, restoring the snapshot doesnt update the UI correctly
		// Until we have that figured out, we need to reload the page.
		location.reload();
	}

	// Include the id in the radio group
	$effect(() => {
		if (focusableId) {
			$focusableIds = [...untrack(() => $focusableIds), focusableId];
			return () => {
				$focusableIds = untrack(() => $focusableIds).filter((id) => id !== focusableId);
			};
		}
	});

	const changes = $derived(
		oplogService.diffWorktree({
			projectId,
			before,
			after
		})
	);

	let showPreviousActions = $state(false);
	let showPreviousActionsTarget = $state<HTMLElement>();
</script>

<ReduxResult {projectId} result={changes.current}>
	{#snippet children(changes, { projectId: _projectId })}
		{#if changes.changes.length > 0 && focusableId}
			<DataContextMenu
				bind:open={showPreviousActions}
				items={[
					[
						{
							label: 'Revert to before',
							onclick: async () => await restore(before, `Reverted to before files changed`)
						},
						{
							label: 'Revert to after',
							onclick: async () => await restore(after, `Reverted to after files changed`)
						}
					]
				]}
				target={showPreviousActionsTarget}
			/>

			<div class="action-item" use:focusable={{ id: focusableId }}>
				<div class="action-item__robot">
					<Icon name="robot" />
				</div>
				<div class="action-item__content">
					<div class="action-item__content__header">
						<div>
							<p class="text-13 text-bold">Files changed</p>
							<span class="text-13 text-greyer"
								><TimeAgo date={new Date(timestamp)} addSuffix /></span
							>
						</div>
						<div bind:this={showPreviousActionsTarget}>
							<Button
								icon="kebab"
								size="tag"
								kind="outline"
								onclick={() => (showPreviousActions = true)}
							/>
						</div>
					</div>

					<div class="action-item__content__metadata">
						<FileList
							{projectId}
							changes={changes.changes}
							listMode="list"
							draggableFiles={false}
							selectionId={{
								type: 'snapshot',
								before,
								after
							}}
							active={selectionId.type === 'snapshot' &&
								selectionId.before === before &&
								selectionId.after === after}
						/>
					</div>
				</div>
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.action-item__robot {
		padding: 4px 6px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.action-item {
		display: flex;

		align-items: flex-start;

		gap: 14px;
	}

	.action-item__content__header {
		display: flex;
		align-items: flex-start;

		> div:first-of-type {
			flex-grow: 1;
		}

		> div {
			display: flex;
			flex-wrap: wrap;

			align-items: center;
			gap: 8px;
		}
	}

	.action-item__content {
		display: flex;

		flex-grow: 1;
		flex-direction: column;
		gap: 8px;
	}

	.action-item__content__metadata {
		width: 100%;

		margin-top: 4px;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.outcome-item {
		display: flex;

		padding: 12px;
		gap: 8px;

		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.text-grey {
		color: var(--clr-text-2);
	}

	.text-darkgrey {
		color: var(--clr-core-ntrl-20);
		text-decoration-color: var(--clr-core-ntrl-20);
	}

	.text-greyer {
		color: var(--clr-text-3);
	}

	.pill {
		padding: 2px 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: 99px;
		background-color: var(--clr-bg-2);
	}
</style>
