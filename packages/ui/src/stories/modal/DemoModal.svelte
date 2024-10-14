<script lang="ts">
	import iconsJson from '$lib/data/icons.json';
	import Button from '$lib/Button.svelte';
	import Modal from '$lib/Modal.svelte';

	interface Props {
		width?: 'default' | 'small' | 'large';
		title?: string;
		icon?: keyof typeof iconsJson;
	}

	const { ...args }: Props = $props();

	let modal = $state<Modal>();
</script>

<Button
	onclick={() => {
		modal?.show();
	}}>Show</Button
>
<Modal bind:this={modal} {...args}>
	<p>Wonderful modal content</p>

	{#snippet controls(close)}
		<Button onclick={() => close()}>Close</Button>
		<Button style="pop" kind="solid" type="submit" onclick={() => console.log('Submit clicked')}
			>Submit</Button
		>
	{/snippet}
</Modal>
