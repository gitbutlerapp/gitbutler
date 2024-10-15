<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Modal from '$lib/Modal.svelte';

	const { ...args }: typeof Modal = $props();

	let modal: Modal;
</script>

<Button
	onclick={() => {
		modal?.show();
	}}>Show</Button
>
<Modal bind:this={modal} {...args} onSubmit={() => console.log('submitted')}>
	A branch with the same name already exists. Do you want to merge this branch into the current
	branch?

	{#snippet controls(close)}
		<Button style="ghost" outline onclick={() => close()}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit" onclick={() => console.log('clicked')}
			>Merge</Button
		>
	{/snippet}
</Modal>
