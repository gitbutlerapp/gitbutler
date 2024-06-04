<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { onMount } from 'svelte';
	import OutClick from 'svelte-outclick';
	import type iconsJson from '$lib/icons/icons.json';

	let dialog: HTMLDialogElement;
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
</script>

<dialog
	class="dialog-wrap"
	class:s-default={width === 'default'}
	class:s-small={width === 'small'}
	class:s-large={width === 'large'}
	bind:this={dialog}
	on:close={close}
>
	{#if open}
		<OutClick on:outclick={close}>
			<div class="dialog">
				<form class="modal-content" on:submit>
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
			</div>
		</OutClick>
	{/if}
</dialog>

<style lang="postcss">
	.dialog-wrap {
		position: relative;
		width: 100%;
		max-height: calc(100vh - 5rem);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-l);
	}

	.dialog {
		display: flex;
		flex-direction: column;
	}

	/* modifiers */

	.s-large {
		max-width: calc(var(--size-64) * 13);
	}

	.s-default {
		max-width: calc(var(--size-64) * 9);
	}

	.s-small {
		max-width: calc(var(--size-64) * 6);
	}

	.modal__header {
		display: flex;
		padding: var(--size-16);
		gap: var(--size-8);
		border-bottom: 1px solid var(--clr-border-2);
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
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}
</style>
