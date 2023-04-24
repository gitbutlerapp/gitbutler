<script lang="ts">
	import { scale } from 'svelte/transition';

	let dialog: HTMLDialogElement;
	let content: HTMLDivElement | null = null;

	let open = false;

	export const show = () => {
		dialog.showModal();
		open = true;
	};
	export const isOpen = () => open;

	export const close = () => {
		dialog.close();
		open = false;
	};

	const handleClick = (event: Event) => {
		if (event.defaultPrevented) return;
		if (!dialog?.open) return;
		const isClickInside = !content || content.contains(event.target as Node | null);
		if (isClickInside) return;
		close();
	};
</script>

<dialog
	on:click={handleClick}
	on:keydown={handleClick}
	class="bg-transparent"
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:close={close}
>
	{#if open}
		<div bind:this={content} class="flex">
			<slot {close} isOpen={open} />
		</div>
	{/if}
</dialog>
