<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { type Snippet } from 'svelte';
	import { onMount } from 'svelte';
	import { run } from 'svelte/legacy';

	interface Props {
		foldable?: boolean;
		foldedAmount?: number | undefined;
		foldedHeight?: string;
		children?: Snippet;
	}

	let {
		foldable = false,
		foldedAmount = undefined,
		foldedHeight = '3rem',
		children
	}: Props = $props();

	let isOpen: boolean = $state(false);
	let el = $state<HTMLElement>();

	let contentHeight = $state<string>();

	function setHeight() {
		if (el) {
			contentHeight = `calc(${pxToRem(el.scrollHeight)} + ${pxToRem(8)})`;
		}
	}

	onMount(() => {
		if (!foldable) return;

		setHeight();
	});

	run(() => {
		if (el) {
			setHeight();
		}
	});
</script>

<div class="snapshot-attachment">
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		bind:this={el}
		onclick={() => {
			if (foldable && !isOpen) {
				isOpen = true;
			}
		}}
		class="snapshot-attachment__content"
		style="max-height: {foldable ? (isOpen ? contentHeight : foldedHeight) : 'auto'}"
	>
		{@render children?.()}
	</div>
	{#if foldable}
		<button
			type="button"
			class="toggle-btn"
			onclick={() => {
				isOpen = !isOpen;
			}}
		>
			<span class="text-11">{isOpen ? 'Fold files' : `Show ${foldedAmount} more`}</span>
			<div class="toggle-btn__icon" style="transform: rotate({isOpen ? '180deg' : '0'})">
				<Icon name="chevron-down-small" />
			</div>
		</button>
	{/if}
</div>

<style lang="postcss">
	.snapshot-attachment {
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		width: 100%;
		overflow: hidden;
	}

	.snapshot-attachment__content {
		display: flex;
		flex-direction: column;
		height: 100%;
		transition: max-height 0.2s ease;
	}

	.toggle-btn {
		position: relative;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 8px;
		color: var(--clr-text-2);
		background-color: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		transition:
			color var(--transition-fast),
			background-color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
			background-color: var(--clr-bg-2);
		}
	}

	.toggle-btn__icon {
		display: flex;
		align-items: center;
	}
</style>
