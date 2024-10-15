<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Modal, { type Props as ModalProps } from '$lib/Modal.svelte';
	import { type SvelteComponent, type Snippet } from 'svelte';

	const { ...args }: ModalProps = $props();

	let modal: SvelteComponent<ModalProps>;

	$effect(() => {
		modal?.show();
	});
</script>

<Button
	onclick={() => {
		modal?.show();
	}}>Show</Button
>
<Modal bind:this={modal} type={args.type} title={args.title} width={args.width}>
	<p>
		A branch with the same name already exists. Do you want to merge this branch into the current
		branch?
	</p>

	{#snippet controls(close)}
		<Button style="ghost" outline onclick={() => close()}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit" onclick={() => console.log('Submit clicked')}
			>Merge</Button
		>
	{/snippet}
</Modal>
