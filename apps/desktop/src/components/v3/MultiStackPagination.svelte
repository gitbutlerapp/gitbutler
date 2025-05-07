<script lang="ts" module>
	export function scrollToLane(el: HTMLElement | undefined, index: number) {
		const laneWidth = el?.offsetWidth ?? 0;

		el?.scrollTo({
			left: laneWidth * index,
			behavior: 'smooth'
		});
	}
</script>

<script lang="ts">
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		length: number;
		activeIndex?: number;
		selectedBranchIndex: number;
		onclick: (index: number) => void;
	};

	let { length, activeIndex = $bindable(), selectedBranchIndex, onclick }: Props = $props();

	function getPaginationTooltip(index: number) {
		if (index === activeIndex) {
			return 'Current branch';
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
				class:active={i === activeIndex}
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
