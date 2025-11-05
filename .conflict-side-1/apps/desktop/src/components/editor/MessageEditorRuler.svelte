<script lang="ts">
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';

	interface Props {
		monospaceFont?: string;
	}

	let { monospaceFont }: Props = $props();

	const uiState = inject(UI_STATE);
	const rulerCountValue = uiState.global.rulerCountValue;

	let lineWidth = $state(0);
</script>

<div
	class="message-ruler-container"
	style:--ruler-position="calc({lineWidth}px - var(--lexical-input-client-padding))"
	style:--ruler-font={monospaceFont || 'var(--font-default)'}
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
		z-index: 1;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		/* background-color: rgba(0, 0, 0, 0.1); */
		pointer-events: none;
	}

	.message-ruler-line {
		position: absolute;
		top: 0;
		bottom: var(--lexical-input-client-toolbar-height);
		left: var(--ruler-position);
		width: 1px;
		background-color: var(--clr-theme-pop-element);
		opacity: 0.5;
	}

	.message-ruler-dummy {
		position: absolute;
		top: 0;
		left: 0;
		padding: var(--lexical-input-client-padding);
		color: red;
		font-size: var(--lexical-input-font-size);
		font-family: var(--ruler-font);
		opacity: 0;
	}
</style>
