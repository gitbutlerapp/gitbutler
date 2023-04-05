<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	import { scale } from 'svelte/transition';

	let dialog: HTMLDialogElement;
	let content: HTMLDivElement | null = null;

	const dispatch = createEventDispatcher<{ close: void }>();

	let open = false;

	export const show = () => {
		open = true;
		console.log('lfkjdsflkdsj');
		dialog.showModal();
	};
	export const hide = () => {
		open = false;
		dialog.close();
		dispatch('close');
	};
	export const isOpen = () => open;

	const handleClick = (event: MouseEvent) => {
		if (content && !content.contains(event.target as Node | null) && !event.defaultPrevented) {
			hide();
		}
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

<svelte:window on:click={handleClick} />

<dialog
	class="@apply flex w-[640px] overflow-hidden rounded-lg border-[0.5px] border-[#3F3F3f] bg-zinc-900/70 p-0 p-0 shadow-lg backdrop-blur-lg"
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:close={hide}
>
	{#if open}
		<div class="flex" bind:this={content}>
			<slot />
		</div>
	{/if}
</dialog>
