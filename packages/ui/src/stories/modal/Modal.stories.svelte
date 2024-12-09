<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import Modal from '$lib/Modal.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / Modal',
		component: Modal,
		args: {},
		argTypes: {}
	});
</script>

<script lang="ts">
	setTemplate(template);
	let modal: ReturnType<typeof Modal>;
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<Button
		onclick={() => {
			modal?.show();
		}}
	>
		Show
	</Button>

	<Modal bind:this={modal} {...args} onSubmit={() => console.log('submitted')}>
		A branch with the same name already exists. Do you want to merge this branch into the current
		branch?

		{#snippet controls(close)}
			<Button style="ghost" outline onclick={() => close()}>Cancel</Button>
			<Button style="pop" kind="solid" type="submit" onclick={() => console.log('clicked')}>
				Merge
			</Button>
		{/snippet}
	</Modal>
{/snippet}

<Story name="Default" />
