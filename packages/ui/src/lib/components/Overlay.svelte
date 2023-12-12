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
	class="dialog-overlay"
	class:show-modal={open}
	class:w-[680px]={width === 'default'}
	class:w-[380px]={width === 'small'}
	class:w-[860px]={width === 'large'}
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
	.dialog-overlay {
		display: flex;
		flex-direction: column;
		position: relative;
		max-height: calc(100vh - 5rem);
		border-radius: var(--radius-l);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		box-shadow: var(--fx-shadow-l);

		&::backdrop {
			background-color: var(--clr-theme-overlay-bg);
		}
	}

	.show-modal {
		display: flex;
	}
</style>
