<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Modal from '../Modal.svelte';
	import { IconClose } from '$lib/components/icons';

	export const show = () => modal.show();
	const hide = () => modal.hide();

	let modal: Modal;
</script>

<Modal on:close bind:this={modal}>
	<div class="flex w-[620px] flex-col text-[#D4D4D8]">
		<div class="mx-4 my-4 flex">
			<div class="flex-grow text-[18px]">
				<slot name="title">Title</slot>
			</div>
			<button on:click={() => modal.hide()}>
				<IconClose class="h-6 w-6" />
			</button>
		</div>
		<p class="mx-4 my-4 text-base">
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
