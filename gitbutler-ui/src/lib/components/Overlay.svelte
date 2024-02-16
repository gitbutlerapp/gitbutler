<script lang="ts">
	import OutClick from 'svelte-outclick';

	let dialog: HTMLDialogElement;

	let open = false;

	export let width: 'default' | 'small' | 'large' = 'default';

	export function show() {
		if (open) return;
		dialog.showModal();
		open = true;
	}
	export function isOpen() {
		open;
	}

	export function close() {
		if (!open) return;
		dialog.close();
		open = false;
	}
</script>

<dialog
	class="dialog"
	class:open-modal={open}
	class:s-default={width == 'default'}
	class:s-small={width == 'small'}
	class:s-large={width == 'large'}
	bind:this={dialog}
	on:close={close}
	on:close
>
	{#if open}
		<OutClick on:outclick={close}>
			<slot {close} isOpen={open} />
		</OutClick>
	{/if}
</dialog>

<style lang="postcss">
	.dialog {
		flex-direction: column;
		position: relative;
		width: 100%;
		max-height: calc(100vh - 5rem);
		border-radius: var(--radius-l);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		box-shadow: var(--fx-shadow-l);

		&::backdrop {
			background-color: rgba(110, 110, 110, 0.2);
		}
	}

	/* modifiers */

	.s-large {
		max-width: calc(var(--space-64) * 13);
	}

	.s-default {
		max-width: calc(var(--space-64) * 9);
	}

	.s-small {
		max-width: calc(var(--space-64) * 6);
	}

	.open-modal {
		display: flex;
	}
</style>
