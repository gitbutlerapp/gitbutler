<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Modal from '../Modal.svelte';
	import { IconClose } from '$lib/components/icons';

	export const show = () => modal.show();
	const hide = () => modal.hide();

	let modal: Modal;
</script>

<Modal on:close bind:this={modal}>
	<div class="flex flex-col text-zinc-400">
		<div class="flex p-4">
			<div class="flex-grow text-[18px] text-zinc-300">
				<slot name="title">Title</slot>
			</div>
			<button on:click={() => modal.hide()}>
				<IconClose class="h-6 w-6" />
			</button>
		</div>
		<p class="p-4 text-base">
			<slot />
		</p>
		<div class="m-4 ml-auto flex gap-4">
			<slot name="controls" {hide} {show}>
				<Button filled on:click={hide}>Cancel</Button>
				<Button filled role="primary" on:click={hide}>Confirm</Button>
			</slot>
		</div>
	</div>
</Modal>
