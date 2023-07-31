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
	class="rounded-lg border-[0.5px] border-light-200 bg-white p-0 backdrop:bg-white/50 dark:border-dark-500 dark:bg-dark-1000 backdrop:dark:bg-black/50"
	style="max-height: calc(100vh - 5rem)"
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
