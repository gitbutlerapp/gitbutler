<script lang="ts">
	import OutClick from 'svelte-outclick';

	let dialog: HTMLDialogElement;

	let open = false;

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

<dialog class="dialog-overlay" bind:this={dialog} on:close={close} on:close>
	{#if open}
		<OutClick on:outclick={close}>
			<div class="flex">
				<slot {close} isOpen={open} />
			</div>
		</OutClick>
	{/if}
</dialog>

<style lang="postcss">
	.dialog-overlay {
		max-height: calc(100vh - 5rem);
		border-radius: var(--radius-l);
		background-color: var(--clr-theme-container-light);

		&::backdrop {
			background-color: var(--clr-theme-overlay-bg);
		}
	}
</style>
