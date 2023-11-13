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

<dialog
	class="rounded-lg border p-0 backdrop:bg-zinc-200/50 backdrop:dark:bg-zinc-700/50"
	style:background-color="var(--bg-surface)"
	style:border-color="var(--border-surface)"
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
