<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Modal from '$lib/Modal.svelte';
	import iconsJson from '$lib/data/icons.json';

	interface Props {
		width?: 'default' | 'small' | 'large';
		title?: string;
		icon?: keyof typeof iconsJson;
	}

	const { ...args }: Props = $props();

	let modal = $state<ReturnType<typeof Modal>>();
</script>

<Button
	onclick={() => {
		modal?.show();
	}}
>
	Show
</Button>
<Modal bind:this={modal} {...args}>
	<p>Wonderful modal content</p>

	{#snippet controls(close)}
		<Button onclick={() => close()}>Close</Button>
	{/snippet}
</Modal>
