<script lang="ts">
	import Tooltip from './Tooltip.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
	let svgContainer: HTMLDivElement | undefined = $state(undefined);
</script>

{#snippet infoTooltip()}
	<div class="info-tooltip-container">
		<div class="arrow-container">
			<div class="arrow"></div>
		</div>
		<div class="info-tooltip">{@render children()}</div>
	</div>
{/snippet}

<Tooltip
	customTooltip={infoTooltip}
	gap={2}
	showOnClick
	position="bottom"
	ignoreElementOnClick={svgContainer}
>
	<div class="svg-container" bind:this={svgContainer}>
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				fill-rule="evenodd"
				clip-rule="evenodd"
				d="M2.5 8C2.5 4.96243 4.96243 2.5 8 2.5C11.0376 2.5 13.5 4.96243 13.5 8C13.5 11.0376 11.0376 13.5 8 13.5C4.96243 13.5 2.5 11.0376 2.5 8ZM1 8C1 4.13401 4.13401 1 8 1C11.866 1 15 4.13401 15 8C15 11.866 11.866 15 8 15C4.13401 15 1 11.866 1 8ZM9 8.5C9 7.94772 8.55229 7.5 8 7.5C7.44772 7.5 7 7.94772 7 8.5L7 10.5C7 11.0523 7.44772 11.5 8 11.5C8.55228 11.5 9 11.0523 9 10.5V8.5ZM9 5.5C9 4.94772 8.55229 4.5 8 4.5C7.44772 4.5 7 4.94772 7 5.5C7 6.05228 7.44772 6.5 8 6.5C8.55229 6.5 9 6.05228 9 5.5Z"
				fill="#867E79"
			/>
		</svg>
	</div>
</Tooltip>

<style lang="postcss">
	.svg-container {
		cursor: pointer;
	}

	.info-tooltip-container {
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	.arrow-container {
		position: relative;
		top: 1px;
		width: 100%;
		height: 10px;
		display: flex;
		justify-content: center;
		overflow: hidden;
		z-index: var(--z-lifted);
	}

	.arrow {
		position: relative;
		top: 4px;
		width: 20px;
		height: 20px;
		transform: rotate(45deg);
		border-radius: 2px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
	}
	.info-tooltip {
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-m);
		padding: 4px 8px;
		border: 1px solid var(--clr-border-2);
		box-shadow: 0px 6px 30px 0px var(--fx-shadow-m);
	}
</style>
