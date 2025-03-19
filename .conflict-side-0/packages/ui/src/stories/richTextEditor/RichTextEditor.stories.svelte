<script module lang="ts">
	import RichTextEditor from '$lib/RichTextEditor.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Editing / RichTextEditor',
		component: RichTextEditor,
		args: {
			styleContext: 'client-editor',
			namespace: 'commit-message',
			markdown: false,
			onError: (error: unknown) => console.error(error),
			placeholder: 'Type your message here…'
		}
	});
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="wrap">
		<RichTextEditor
			namespace={args.namespace || 'commit-message'}
			markdown={args.markdown || false}
			onError={args.onError || console.error}
			styleContext={args.styleContext || 'client-editor'}
			placeholder={args.placeholder || 'Type your message here…'}
		/>
	</div>
{/snippet}

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
	}
</style>
