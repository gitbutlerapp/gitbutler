<script lang="ts" module>
	export function scrollToLane(el: HTMLElement | undefined, index: number) {
		if (!el) return;
		const laneWidth = el?.offsetWidth ?? 0;
		el?.scrollTo({
			left: laneWidth * index,
			behavior: 'smooth'
		});
	}
</script>

<script lang="ts">
	import { Tooltip } from '@gitbutler/ui';

	type Props = {
		length: number;
		visibleIndexes: number[];
		isCreateNewVisible?: boolean;
		selectedBranchIndex: number;
		onPageClick: (index: number) => void;
		onCreateNewClick: () => void;
	};

	let {
		length,
		visibleIndexes = $bindable(),
		isCreateNewVisible = $bindable(),
		selectedBranchIndex,
		onPageClick,
		onCreateNewClick
	}: Props = $props();

	function getPaginationTooltip(index: number) {
		if (visibleIndexes.includes(index)) {
			return 'In view';
		} else if (index === selectedBranchIndex) {
			return 'Selected';
		} else {
			return 'Scroll to lane';
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
				onclick={() => onPageClick(i)}
				onkeydown={(event) => {
					if (event.key === 'Enter' || event.key === ' ') {
						event.preventDefault();
						onPageClick(i);
					}
				}}
			></div>
		</Tooltip>
	{/each}
	<Tooltip text="Create new branch">
		<div
			role="button"
			tabindex="0"
			class="create-new"
			class:create-new-visible={isCreateNewVisible}
			onclick={onCreateNewClick}
			onkeydown={(event) => {
				if (event.key === 'Enter' || event.key === ' ') {
					event.preventDefault();
					onCreateNewClick();
				}
			}}
		>
			<svg width="9" height="8" viewBox="0 0 9 8" fill="none" xmlns="http://www.w3.org/2000/svg">
				<path
					d="M5.49474 0.5V7.5M9 3.99474L2 3.99474"
					stroke="var(--clr-text-2)"
					stroke-width="2"
				/>
			</svg>
		</div>
	</Tooltip>
</div>

<style lang="postcss">
	.pagination {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 6px;
		gap: 2px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.pagination-dot {
		width: 8px;
		height: 8px;
		border-radius: var(--radius-s);
		background: var(--clr-text-2);
		cursor: pointer;
		opacity: 0.4;
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

	.create-new {
		display: flex;
		opacity: 0.5;
		transition: opacity var(--transition-fast);
	}

	.create-new-visible {
		opacity: 1;
	}
</style>
