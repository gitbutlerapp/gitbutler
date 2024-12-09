<script module lang="ts">
	import Textarea from '$lib/Textarea.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	function handleDescriptionKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			console.log('keyboard', e.key);
			e.preventDefault();
			return;
		}

		if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
			console.log('keyboard', e.key);
			e.preventDefault();
			return;
		}
	}

	const { Story } = defineMeta({
		title: 'Inputs / Textarea',
		component: Textarea
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="wrapper">
		<Textarea
			{...args}
			onkeydown={handleDescriptionKeyDown}
			onfocus={(e) => {
				console.log('focus', e);
			}}
		/>
	</div>
{/snippet}

<Story name="Playground" />

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		max-width: 300px;
		gap: 12px;
	}
</style>
