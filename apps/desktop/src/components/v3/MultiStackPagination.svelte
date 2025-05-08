<script lang="ts" module>
	export function scrollToLane(
		el: HTMLElement | undefined,
		index: number,
		direction: 'horz' | 'vert'
	) {
		if (!el) return;
		if (direction === 'vert') {
			const laneHeight = el?.offsetHeight ?? 0;
			el?.scrollTo({
				top: laneHeight * index,
				behavior: 'smooth'
			});
		} else {
			const laneWidth = el?.offsetWidth ?? 0;
			el?.scrollTo({
				left: laneWidth * index,
				behavior: 'smooth'
			});
		}
	}
</script>

<script lang="ts">
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		length: number;
		visibleIndexes: number[];
		selectedBranchIndex: number;
		onclick: (index: number) => void;
	};

	let { length, visibleIndexes = $bindable(), selectedBranchIndex, onclick }: Props = $props();

	function getPaginationTooltip(index: number) {
		if (visibleIndexes.includes(index)) {
			return `Branch ${index + 1}`;
		} else if (index === selectedBranchIndex) {
			return 'Selected branch';
		} else {
			return `Switch to branch ${index + 1}`;
		}
	}
</script>

<div class="pagination">
	{#each Array(length) as _, i}
		<Tooltip text={getPaginationTooltip(i)}>
			<div
				role="button"
				tabindex="0"
				class="pagination-dot"
				class:active={visibleIndexes.includes(i)}
				class:selected-branch={i === selectedBranchIndex}
				onclick={() => onclick(i)}
				onkeydown={(event) => {
					if (event.key === 'Enter' || event.key === ' ') {
						event.preventDefault();
						onclick(i);
					}
				}}
			></div>
		</Tooltip>
	{/each}
</div>

<style lang="postcss">
	.pagination {
		border-radius: var(--radius-ml);
		display: flex;
		justify-content: center;
		align-items: center;
		gap: 2px;
		padding: 6px;
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.pagination-dot {
		width: 8px;
		height: 8px;
		border-radius: var(--radius-s);
		background: var(--clr-text-2);
		opacity: 0.4;
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&:hover {
			opacity: 0.7;
		}

		&.active {
			opacity: 1;
		}
		&.selected-branch {
			background: var(--clr-selected-in-focus-element);
		}
	}
</style>
