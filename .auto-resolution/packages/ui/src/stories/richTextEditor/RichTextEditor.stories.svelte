<script module lang="ts">
	import RichTextEditor from '$lib/RichTextEditor.svelte';
	import Formatter from '$lib/richText/plugins/Formatter.svelte';
	import FormattingBar from '$lib/richText/tools/FormattingBar.svelte';
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
	let formatter = $state<ReturnType<typeof Formatter>>();
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="wrap">
		<FormattingBar bind:formatter />
		<div class="text-input">
			<RichTextEditor
				namespace={args.namespace || 'commit-message'}
				markdown={args.markdown || false}
				onError={args.onError || console.error}
				styleContext={args.styleContext || 'client-editor'}
				placeholder={args.placeholder || 'Type your message here…'}
			>
				{#snippet plugins()}
					<Formatter bind:this={formatter} />
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
		max-width: 600px;
		gap: 10px;
	}

	.text-input {
		display: flex;
		flex-direction: column;
		height: 140px;
	}
</style>
