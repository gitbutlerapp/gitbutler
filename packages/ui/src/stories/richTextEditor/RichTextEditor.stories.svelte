<script module lang="ts">
	import RichTextEditor from '$lib/RichTextEditor.svelte';
	import Formatter from '$lib/richText/plugins/Formatter.svelte';
	import FormattingButton from '$lib/richText/tools/FormattingButton.svelte';
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
		<div class="formatting__options">
			<div class="formatting__options-wrap">
				<div class="formatting__group">
					<FormattingButton
						iconName="text-bold"
						isActivated={formatter?.imports.isBold}
						tooltip="Bold"
						onClick={() => formatter?.format('text-bold')}
					/>
					<FormattingButton
						iconName="text-italic"
						isActivated={formatter?.imports.isItalic}
						tooltip="Italic"
						onClick={() => formatter?.format('text-italic')}
					/>
					<FormattingButton
						iconName="text-underline"
						isActivated={formatter?.imports.isUnderline}
						tooltip="Underline"
						onClick={() => formatter?.format('text-underline')}
					/>
					<FormattingButton
						iconName="text-strikethrough"
						isActivated={formatter?.imports.isStrikethrough}
						tooltip="Strikethrough"
						onClick={() => formatter?.format('text-strikethrough')}
					/>
					<FormattingButton
						iconName="text-code"
						isActivated={formatter?.imports.isCode}
						tooltip="Code"
						onClick={() => formatter?.format('text-code')}
					/>
					<FormattingButton
						iconName="text-quote"
						isActivated={formatter?.imports.isQuote}
						tooltip="Quote"
						onClick={() => formatter?.format('text-quote')}
					/>
					<FormattingButton
						iconName="text-link"
						isActivated={formatter?.imports.isLink}
						tooltip="Link"
						onClick={() => formatter?.format('text-link')}
					/>
				</div>
				<div class="formatting__group">
					<FormattingButton
						iconName="text"
						isActivated={formatter?.imports.isNormal}
						tooltip="Normal text"
						onClick={() => formatter?.format('text')}
					/>
					<FormattingButton
						iconName="text-h1"
						isActivated={formatter?.imports.isH1}
						tooltip="Heading 1"
						onClick={() => formatter?.format('text-h1')}
					/>
					<FormattingButton
						iconName="text-h2"
						isActivated={formatter?.imports.isH2}
						tooltip="Heading 2"
						onClick={() => formatter?.format('text-h2')}
					/>
					<FormattingButton
						iconName="text-h3"
						isActivated={formatter?.imports.isH3}
						tooltip="Heading 3"
						onClick={() => formatter?.format('text-h3')}
					/>
					<FormattingButton
						iconName="bullet-list"
						tooltip="Unordered list"
						onClick={() => formatter?.format('bullet-list')}
					/>
					<FormattingButton
						iconName="number-list"
						tooltip="Ordered list"
						onClick={() => formatter?.format('number-list')}
					/>
					<FormattingButton
						iconName="checklist"
						tooltip="Check list"
						onClick={() => formatter?.format('checklist')}
					/>
				</div>
			</div>
		</div>
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
{/snippet}

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
		flex-direction: column;
	}

	.formatting__group {
		display: flex;
	}

	.formatting__options-wrap {
		display: flex;
	}
</style>
