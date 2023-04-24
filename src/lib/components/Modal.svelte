<script lang="ts">
	import { scale } from 'svelte/transition';

	let dialog: HTMLDialogElement;
	let content: HTMLDivElement | null = null;

	let open = false;

	export const show = () => {
		open = true;
		dialog.showModal();
	};
	export const isOpen = () => open;

	const close = () => {
		open = false;
		dialog.close();
	};

	const handleClick = (event: MouseEvent) => {
		if (event.defaultPrevented) return;
		const isClickInside = !content || content.contains(event.target as Node | null);
		if (isClickInside) return;
		close();
	};
</script>

<!--
@component
In most cases, you should use the Dialog component, which builds on top of this, instead of this one.
This is a base Modal component which makes sure that all mouse and keyboard events are handled correctly.
It does minimal styling. A close event is fired when the modal is closed.

- Usage:
  ```tsx
<Modal>
	your content slotted in
</Modal>
  ```
-->
<!-- test -->
<svelte:window on:click={handleClick} />

<dialog class="bg-transparent" in:scale={{ duration: 150 }} bind:this={dialog} on:close={close}>
	{#if open}
		<div
			bind:this={content}
			class="flex overflow-hidden rounded-lg border-[0.5px] border-[#3F3F3f] bg-zinc-900/70 p-0 shadow-lg backdrop-blur-lg"
		>
			<slot {close} isOpen={open} />
		</div>
	{/if}
</dialog>
