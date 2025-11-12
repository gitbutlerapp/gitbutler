<script module lang="ts">
	import RichTextEditor from '$lib/richText/RichTextEditor.svelte';
	import Formatter from '$lib/richText/plugins/Formatter.svelte';
	import UpDownPlugin from '$lib/richText/plugins/UpDownPlugin.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

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
</script>

<Story name="default">
	{#snippet template(args)}
		<div class="wrap">
			<div class="text-input">
				<RichTextEditor
					namespace={args.namespace || 'commit-message'}
					markdown={args.markdown || false}
					onError={args.onError || console.error}
					styleContext={args.styleContext || 'client-editor'}
					placeholder={args.placeholder || 'Type your message here…'}
					wrapCountValue={args.wrapCountValue}
					initialText={args.initialText}
				>
					{#snippet plugins()}
						<Formatter bind:this={formatter} />
						<UpDownPlugin historyLookup={async (offset) => `offset ${offset}`} />
					{/snippet}
				</RichTextEditor>
			</div>
		</div>
	{/snippet}
</Story>

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
