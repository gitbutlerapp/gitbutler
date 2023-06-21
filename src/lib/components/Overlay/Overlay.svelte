<script lang="ts">
	import { scale } from 'svelte/transition';
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

<dialog
	class="bg-transparent"
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:close={close}
	on:close
>
	{#if open}
		<OutClick on:outclick={close}>
			<div class="flex">
				<slot {close} isOpen={open} />
			</div>
		</OutClick>
	{/if}
</dialog>
