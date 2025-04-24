<script lang="ts">
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const uiState = getContext(UiState);
	const rulerCountValue = uiState.global.rulerCountValue;

	let lineWidth = $state(0);
</script>

<div
	class="message-ruler-container"
	style:--ruler-position="calc({lineWidth}px - var(--lexical-input-client-padding))"
>
	<div class="message-ruler-dummy" bind:clientWidth={lineWidth}>
		<!-- Create a dummy amount of text to measure the width -->
		{#if rulerCountValue.current > 0}
			{#each Array(rulerCountValue.current) as _}
				i
			{/each}
		{/if}
	</div>
	<div class="message-ruler-line"></div>
</div>

<style lang="postcss">
	.message-ruler-container {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
		z-index: 1;
		/* background-color: rgba(0, 0, 0, 0.1); */
	}

	.message-ruler-line {
		position: absolute;
		top: 0;
		left: var(--ruler-position);
		width: 1px;
		height: calc(100% - var(--lexical-input-client-toolbar-height));
		background-color: var(--clr-theme-pop-element);
		opacity: 0.5;
	}

	.message-ruler-dummy {
		position: absolute;
		top: 0;
		left: 0;
		opacity: 0;
		font-family: var(--fontfamily-mono);
		font-size: var(--lexical-input-font-size);
		padding: var(--lexical-input-client-padding);
	}
</style>
