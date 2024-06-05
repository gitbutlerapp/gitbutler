<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { onMount } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';

	let dialog: HTMLDialogElement;
	let modalForm: HTMLFormElement;
	let item: any;
	let open = false;

	export let width: 'default' | 'small' | 'large' = 'default';
	export let title: string | undefined = undefined;
	export let icon: keyof typeof iconsJson | undefined = undefined;

	export function show(newItem?: any) {
		item = newItem;
		dialog.showModal();
		open = true;
	}

	export function close() {
		item = undefined;
		dialog.close();
		open = false;
	}

	onMount(() => {
		document.body.appendChild(dialog);
	});

	function handleClickOutside(e: MouseEvent) {
		if (!modalForm.contains(e.target as Node)) {
			dialog.close();
		}
	}
</script>

<svelte:window on:click={handleClickOutside} />

<dialog
	class:s-default={width === 'default'}
	class:s-small={width === 'small'}
	class:s-large={width === 'large'}
	bind:this={dialog}
	on:close={close}
>
	{#if open}
		<form class="modal-content" on:submit bind:this={modalForm}>
			{#if title}
				<div class="modal__header">
					{#if icon}
						<Icon name={icon} />
					{/if}
					<h2 class="text-base-14 text-semibold">
						{title}
					</h2>
				</div>
			{/if}

			<div class="modal__body custom-scrollbar">
				<slot {item} {close} />
			</div>

			{#if $$slots.controls}
				<div class="modal__footer">
					<slot name="controls" {item} {close} />
				</div>
			{/if}
		</form>
	{/if}
</dialog>

<style lang="postcss">
	dialog {
		display: flex;
		flex-direction: column;
		max-height: calc(100vh - 80px);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-l);
	}

	/* modifiers */

	.s-large {
		width: 840px;
	}

	.s-default {
		width: 580px;
	}

	.s-small {
		width: 380px;
	}

	.modal__header {
		display: flex;
		padding: 16px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.modal__body {
		overflow: auto;
		padding: 16px;
	}

	.modal__footer {
		display: flex;
		width: 100%;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}
</style>
