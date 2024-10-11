<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
		action: Snippet;
		count: number;
	}

	const { children, action, count }: Props = $props();

	let isOpen = $state(count === 1);

	function toggle() {
		isOpen = !isOpen;
	}
</script>

<button class="accordion" onclick={toggle}>
	{#if count !== 1}
		<div class="accordion-row header">
			<div class="accordion-row__line dots">
				{#if !isOpen}
					{#each new Array(count) as _, idx}
						<svg
							width="14"
							height="14"
							viewBox="0 0 14 14"
							class="upstream-dot"
							style="--dot: {idx + 1}; --dotCount: {count + 1};"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<rect
								x="1.76782"
								y="1.76764"
								width="10.535"
								height="10.535"
								rx="3"
								stroke-width="2"
							/>
						</svg>
					{/each}
				{/if}
			</div>
			<div class="accordion-row__right">
				<h5 class="text-13 text-body text-semibold title">Upstream commits</h5>
				<Icon name={isOpen ? 'chevron-up' : 'chevron-down'} />
			</div>
		</div>
	{/if}
	{#if isOpen}
		<div class="accordion-children">
			{@render children()}
		</div>

		<div class="accordion-row unthemed">
			<div class="accordion-row__line"></div>
			<div class="accordion-row__right">
				{@render action()}
			</div>
		</div>
	{/if}
</button>

<style>
	.accordion {
		position: relative;
		display: flex;
		flex-direction: column;
		background-color: var(--clr-theme-warn-bg);

		&:focus {
			outline: none;
		}
	}

	.accordion-row {
		display: flex;
		width: 100%;
		min-height: 44px;
		align-items: stretch;
		text-align: left;
		border-bottom: 1px solid var(--clr-border-2);

		&.unthemed {
			background-color: var(--clr-bg-1);
		}

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		& .accordion-row__line {
			width: 2px;
			margin: 0 22px;
			background-color: var(--clr-commit-upstream);

			&.dots {
				place-items: center;
			}

			& .upstream-dot {
				position: absolute;
				fill: var(--clr-commit-upstream);
				stroke: var(--clr-theme-warn-bg);
				transform: translateX(-6px) translateY(calc(var(--dot) * 7px)) rotate(45deg);
			}
		}

		& .accordion-row__right {
			display: flex;
			flex: 1;
			margin: 0 8px;
			align-items: center;
			color: var(--clr-text-2);
		}

		& .title {
			flex: 1;
			display: block;
			color: var(--clr-text-1);
			width: 100%;
		}
	}

	.accordion-children {
		display: flex;
		flex-direction: column;
		width: 100%;
		min-height: 44px;
		align-items: stretch;
		text-align: left;
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
