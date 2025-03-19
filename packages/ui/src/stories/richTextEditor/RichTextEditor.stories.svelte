<script module lang="ts">
	import RichTextEditor from '$lib/RichTextEditor.svelte';
	import FormattingPopup from '$lib/richText/tools/FormattingPopup.svelte';
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
		<div class="editor-wrap">
			<RichTextEditor
				namespace={args.namespace || 'commit-message'}
				markdown={args.markdown || false}
				onError={args.onError || console.error}
				styleContext={args.styleContext || 'client-editor'}
				placeholder={args.placeholder || 'Type your message here…'}
			>
				{#snippet toolBar()}
					<FormattingPopup />
				{/snippet}
			</RichTextEditor>
		</div>
	</div>
{/snippet}

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
		flex-direction: column;
		gap: 8px;
		max-width: 600px;
	}
	.editor-wrap {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		flex: 1;
	}
</style>
