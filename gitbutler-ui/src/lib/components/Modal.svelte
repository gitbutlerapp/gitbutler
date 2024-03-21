<script lang="ts">
	import Overlay from './Overlay.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export function show(newItem?: any) {
		item = newItem;
		modal.show();
	}
	export function close() {
		item = undefined;
		modal.close();
	}

	export let width: 'default' | 'small' | 'large' = 'default';
	export let title: string | undefined = undefined;
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let hoverText: string | undefined = undefined;

	let item: any;
	let modal: Overlay;
</script>

<Overlay bind:this={modal} let:close on:close {width}>
	{#if title}
		<div class="modal__header">
			<div class="modal__header__content" class:adjust-header={$$slots.header_controls}>
				{#if icon}
					<Icon name={icon} />
				{/if}
				<h2 class="text-base-14 text-semibold" title={hoverText}>
					{title}
				</h2>
			</div>
			{#if $$slots.header_controls}
				<div class="modal__header__actions">
					<slot name="header_controls" />
				</div>
			{/if}
		</div>
	{/if}

	<div class="modal__body custom-scrollbar">
		<slot {item} />
	</div>

	{#if $$slots.controls}
		<div class="modal__footer">
			<slot name="controls" {item} {close} />
		</div>
	{/if}
</Overlay>

<style lang="postcss">
	.modal__header {
		display: flex;
		padding: var(--size-16);
		gap: var(--size-8);
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}

	.modal__header__content {
		display: flex;
		gap: var(--size-8);
		flex: 1;
	}

	.modal__header__actions {
		display: flex;
		gap: var(--size-8);
	}

	.modal__body {
		overflow: auto;
		padding: var(--size-16);
	}

	.modal__footer {
		display: flex;
		width: 100%;
		justify-content: flex-end;
		gap: var(--size-8);
		padding: var(--size-16);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
	}

	.adjust-header {
		margin-top: var(--size-6);
	}
</style>
