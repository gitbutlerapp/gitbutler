<script lang="ts">
	import { BranchStack } from '$lib/branches/branch';
	import { getContextStore } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	import type { Persisted } from '@gitbutler/shared/persisted';

	interface Props {
		uncommittedChanges?: number;
		isLaneCollapsed: Persisted<boolean>;
	}

	const { uncommittedChanges = 0, isLaneCollapsed }: Props = $props();

	const branchStore = getContextStore(BranchStack);
	const stack = $derived($branchStore);
	const nonArchivedSeries = $derived(stack.validSeries.filter((s) => !s.archived));

	function expandLane() {
		$isLaneCollapsed = false;
	}

	let headerInfoHeight = $state(0);
</script>

<div
	class="card collapsed-lane"
	class:collapsed-lane_target-branch={stack.selectedForChanges}
	onkeydown={(e) => e.key === 'Enter' && expandLane()}
	tabindex="0"
	role="button"
>
	<div class="collapsed-lane__actions">
		<div class="draggable" data-drag-handle>
			<Icon name="draggable" />
		</div>
		<Button kind="outline" icon="unfold-lane" tooltip="Expand lane" onclick={expandLane} />
	</div>

	<div class="collapsed-lane__info-wrap" bind:clientHeight={headerInfoHeight}>
		<div class="collapsed-lane__info" style="width: {headerInfoHeight}px">
			<div class="collapsed-lane__label-wrap">
				{#if uncommittedChanges > 0}
					<Badge size="tag" style="warning" kind="soft" tooltip="Uncommitted changes">
						{uncommittedChanges}
						{uncommittedChanges === 1 ? 'change' : 'changes'}
					</Badge>
				{/if}
				<SeriesLabelsRow series={nonArchivedSeries.map((s) => s.name)} />
			</div>

			<div class="collapsed-lane__info__details">
				{#if stack.selectedForChanges}
					<Badge style="pop" kind="soft" size="tag" icon="target">Default lane</Badge>
				{/if}
			</div>
		</div>
	</div>
</div>

<style>
	.draggable {
		display: flex;
		align-items: center;
		height: fit-content;
		padding: 2px 2px 0 0;
		color: var(--clr-scale-ntrl-50);
		cursor: grab;
		transition: color var(--transition-slow);

		&:hover {
			color: var(--clr-scale-ntrl-40);
		}
	}

	.collapsed-lane {
		align-items: center;
		width: 48px;
		height: 100%;
		padding: 8px 8px 20px;
		overflow: hidden;
		gap: 8px;
		cursor: default;

		&:focus-within {
			outline: none;
		}
	}

	.collapsed-lane_target-branch {
		border-color: var(--clr-theme-pop-element);
	}

	.collapsed-lane__actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
	}

	/*  */

	.collapsed-lane__info-wrap {
		display: flex;
		height: 100%;
	}

	.collapsed-lane__info {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		transform: rotate(-90deg);
		direction: ltr;
	}

	/*  */

	.collapsed-lane__info__details {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		gap: 4px;
	}

	.collapsed-lane__label-wrap {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 12px;
	}
</style>
