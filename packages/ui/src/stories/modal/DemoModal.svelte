<script lang="ts">
	import iconsJson from '$lib/icon/icons.json';
	import Button from '$lib/inputs/Button.svelte';
	import Modal from '$lib/modal/Modal.svelte';

	interface Props {
		width?: 'default' | 'small' | 'large';
		title?: string;
		icon?: keyof typeof iconsJson;
	}

	const { ...args }: Props = $props();

	let modal = $state<Modal>();
</script>

<Button on:click={() => modal?.show()}>Show</Button>
<Modal bind:this={modal} {...args}>
	<p>Wonderful modal content</p>

	{#snippet controls(close)}
		<Button on:click={() => close()}>Close</Button>
	{/snippet}
</Modal>
