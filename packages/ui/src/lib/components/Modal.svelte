<script lang="ts">
	import Overlay from './Overlay.svelte';
	import type { ComponentType } from 'svelte';

	export function show(newItem?: any) {
		item = newItem;
		modal.show();
	}
	export function close() {
		item = undefined;
		modal.close();
	}

	export let width: 'default' | 'small' | 'large' = 'default';

	let item: any;
	let modal: Overlay;
</script>

<Overlay bind:this={modal} let:close on:close {width}>
	{#if $$slots.title}
		<div class="modal__header">
			<div class="modal__header__content" class:adjust-header={$$slots.header_controls}>
				{#if $$slots.icon}
					<slot name="icon" />
				{/if}
				<h2 class="text-base-14 text-semibold">
					<slot name="title" />
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
		padding: var(--space-16);
		gap: var(--space-8);
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}

	.modal__header__content {
		display: flex;
		gap: var(--space-8);
		flex: 1;
	}

	.modal__header__actions {
		display: flex;
		gap: var(--space-8);
	}

	.modal__body {
		overflow: auto;
		padding: var(--space-16);
	}

	.modal__footer {
		display: flex;
		width: 100%;
		justify-content: flex-end;
		gap: var(--space-8);
		padding: var(--space-16);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
	}

	.adjust-header {
		margin-top: var(--space-6);
	}
</style>
