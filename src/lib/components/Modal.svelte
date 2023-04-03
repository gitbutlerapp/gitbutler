<script lang="ts">
	import { scale } from 'svelte/transition';
	let dialog: HTMLDialogElement;

	export const show = () => {
		dialog.showModal();
	};
	export const hide = () => {
		dialog.close();
	};
</script>

<!--
@component
In most cases, you should use the Dialog component, which builds on top of this, instead of this one.
This is a base Modal component which makes sure that all mouse and keyboard events are handled correctly.
It does minimal styling. A close event is fired when the modal is closed.

- Usage:
  ```tsx
<Modal on:close>
	your content slotted in
</Modal>
  ```
-->

<!-- svelte-ignore a11y-click-events-have-key-events -->
<dialog
	class="modal "
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:click|self={hide}
	on:close
>
	<slot />
</dialog>

<style>
	.modal {
		@apply flex w-[640px] overflow-hidden rounded-lg border-[0.5px] border-[#3F3F3f] bg-zinc-900/70 p-0 p-0 shadow-lg shadow-lg;
		backdrop-filter: blur(10px);
	}
</style>
