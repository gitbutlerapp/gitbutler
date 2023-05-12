<script lang="ts">
	import { scale } from 'svelte/transition';
	import clickOutside from './click_outside';

	let dialog: HTMLDialogElement;

	let open = false;

	export const show = () => {
		if (open) return;
		dialog.showModal();
		open = true;
	};
	export const isOpen = () => open;

	export const close = () => {
		if (!open) return;
		dialog.close();
		open = false;
	};
</script>

<dialog
	class="bg-transparent"
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:close={close}
	on:close
>
	{#if open}
		<div use:clickOutside on:outclick={close} class="flex">
			<slot {close} isOpen={open} />
		</div>
	{/if}
</dialog>
