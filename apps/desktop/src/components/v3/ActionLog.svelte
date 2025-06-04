<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ActionLogFilesChanged from '$components/v3/ActionLogFilesChanged.svelte';
	import ActionLogItem from '$components/v3/ActionLogMcpItem.svelte';
	import ActionService from '$lib/actions/actionService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { untrack } from 'svelte';
	import type { ButlerAction } from '$lib/actions/types';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		selectionId: SelectionId;
	};

	const { projectId, selectionId }: Props = $props();

	const [actionService] = inject(ActionService);

	let requiredPages = $state([1]);
	const pages = $derived(requiredPages.map((page) => actionService.listActions(projectId, page)));

	function previous(pi: number, i: number, lastInPage: boolean, last: boolean) {
		if (last) return;
		if (lastInPage) {
			return pages[pi + 1]?.current?.data?.actions.at(0);
		} else {
			return pages[pi]?.current?.data?.actions.at(i + 1);
		}
	}

	function loadNextPage() {
		requiredPages.push(untrack(() => requiredPages).at(-1)! + 1);
	}
</script>

<div class="action-log-wrap">
	<div class="action-log">
		<div class="action-log__header">
			<h2 class="text-16 text-semibold">Butler Actions</h2>
		</div>
		<div class="scrollable">
			{#each pages as page, pi}
				<ReduxResult {projectId} result={page.current}>
					{#snippet children(actions)}
						{#each actions.actions as action, i (action.id)}
							{@const lastInPage = i === actions.actions.length - 1}
							{@const last = lastInPage && page === pages.at(-1)!}
							{@const p = previous(pi, i, lastInPage, last)}
							{#if action.action.type === 'mcpAction'}
								<ActionLogItem
									{projectId}
									action={action as ButlerAction & { action: { type: 'mcpAction' } }}
									{last}
									{loadNextPage}
								/>
							{/if}
							{#if p}
								{@const after =
									action.action.type === 'mcpAction'
										? action.action.subject.snapshotBefore
										: action.action.subject.snapshot}
								{@const before =
									p.action.type === 'mcpAction'
										? p.action.subject.snapshotAfter
										: p.action.subject.snapshot}
								<ActionLogFilesChanged
									{projectId}
									{before}
									{after}
									{selectionId}
									timestamp={action.createdAt}
								/>
							{/if}
						{/each}
					{/snippet}
				</ReduxResult>
			{/each}
		</div>
	</div>
</div>

<style lang="postcss">
	.action-log-wrap {
		flex-grow: 1;

		overflow: hidden;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.action-log__header {
		padding: 16px;

		border-bottom: 1px solid var(--clr-border-2);
	}

	.action-log {
		display: flex;
		flex-direction: column;

		height: 100%;
	}

	.scrollable {
		display: flex;
		flex-grow: 1;
		flex-direction: column-reverse;

		padding: 16px;

		overflow: auto;

		gap: 20px;
	}
</style>
