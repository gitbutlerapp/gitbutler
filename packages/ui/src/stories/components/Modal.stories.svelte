<script module lang="ts">
	import Button from '$components/Button.svelte';
	import Modal from '$components/Modal.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / Modal',
		args: {},
		argTypes: {}
	});
</script>

<script lang="ts">
	let modal: ReturnType<typeof Modal>;
</script>

<Story name="default">
	{#snippet template(args)}
		<Button
			onclick={() => {
				modal?.show();
			}}
		>
			Show
		</Button>

		<Modal
			bind:this={modal}
			{...args}
			onSubmit={() => {
				// eslint-disable-next-line no-console
				console.log('submitted');
			}}
		>
			A branch with the same name already exists. Do you want to merge this branch into the current
			branch?
			{#snippet controls(close)}
				<Button kind="outline" onclick={() => close()}>Cancel</Button>
				<Button
					style="pop"
					type="submit"
					onclick={() => {
						// eslint-disable-next-line no-console
						console.log('clicked');
					}}>Merge</Button
				>
			{/snippet}
		</Modal>
	{/snippet}
</Story>

<Story name="Playground" />
