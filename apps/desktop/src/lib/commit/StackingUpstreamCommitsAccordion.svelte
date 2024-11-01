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

<div class="accordion">
	{#if count !== 1}
		<button type="button" class="accordion-row header" onclick={toggle}>
			<div class="accordion-row__line">
				<div class="dots">
					{#if !isOpen}
						{#each new Array(count) as _, idx}
							<svg
								viewBox="0 0 14 14"
								class="upstream-dot"
								style="--dot: {idx + 1}"
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
			</div>
			<div class="accordion-row__right">
				<h5 class="text-13 text-body text-semibold title">Upstream commits</h5>
				<Icon name={isOpen ? 'chevron-up' : 'chevron-down'} />
			</div>
		</button>
	{/if}
	{#if isOpen}
		<div class="accordion-children">
			{@render children()}
		</div>

		<div class="accordion-row unthemed">
			<div class="accordion-row__line"></div>
			<div class="accordion-row__actions">
				{@render action()}
			</div>
		</div>
	{/if}
</div>

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

		&:not(:global(:last-child)) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		& .accordion-row__line {
			position: relative;
			width: 2px;
			margin: 0 22px 0 20px;
			background-color: var(--clr-commit-upstream);
			--dots-y-shift: -8px;

			& .upstream-dot {
				width: 14px;
				height: 14px;
				fill: var(--clr-commit-upstream);
				stroke: var(--clr-theme-warn-bg);
				transform: rotate(45deg);
				margin-top: var(--dots-y-shift);
			}

			& .dots {
				position: absolute;

				top: calc(50% - (var(--dots-y-shift) / 2));
				left: 50%;
				transform: translate(-50%, -50%);
			}
		}

		& .accordion-row__right {
			display: flex;
			flex: 1;
			padding-right: 14px;
			align-items: center;
			color: var(--clr-text-2);
		}

		& .accordion-row__actions {
			display: flex;
			flex-direction: column;
			flex: 1;
			padding: 14px 14px 14px 0;
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
