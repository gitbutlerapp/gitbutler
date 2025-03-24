<script lang="ts">
	import { showError } from '$lib/notifications/toasts';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import Formatter from '@gitbutler/ui/richText/plugins/Formatter.svelte';
	import GiphyPlugin from '@gitbutler/ui/richText/plugins/GiphyPlugin.svelte';
	import FormattingBar from '@gitbutler/ui/richText/tools/FormattingBar.svelte';

	interface Props {
		markdown: boolean;
		initialValue?: string;
	}

	let { markdown = $bindable(), initialValue }: Props = $props();

	let composer = $state<ReturnType<typeof RichTextEditor>>();
	let formatter = $state<ReturnType<typeof Formatter>>();

	export async function getPlaintext(): Promise<string | undefined> {
		return composer?.getPlaintext();
	}
</script>

<div class="editor-wrapper">
	<div class="editor-header">
		<div class="editor-tabs">
			<button
				type="button"
				class="text-13 text-semibold editor-tab"
				class:active={!markdown}
				onclick={() => {
					markdown = false;
				}}>Plain</button
			>
			<button
				type="button"
				class="text-13 text-semibold editor-tab"
				class:active={markdown}
				onclick={() => {
					markdown = true;
				}}>Rich-text Editor</button
			>
		</div>
		<FormattingBar bind:formatter />
	</div>

	<div role="presentation" class="message-editor-wrapper">
		<RichTextEditor
			styleContext="client-editor"
			namespace="CommitMessageEditor"
			placeholder="Your commit message"
			bind:this={composer}
			{markdown}
			onError={(e) => showError('Editor error', e)}
			initialText={initialValue}
		>
			{#snippet plugins()}
				<Formatter bind:this={formatter} />
				<GiphyPlugin />
			{/snippet}
		</RichTextEditor>
	</div>
</div>

<style lang="postcss">
	.editor-wrapper {
		display: flex;
		flex-direction: column;
		flex: 1;
		background-color: var(--clr-bg-1);
	}

	.editor-header {
		position: relative;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.editor-tabs {
		display: flex;
	}

	.editor-tab {
		position: relative;
		color: var(--clr-text-2);
		padding: 10px;
		background-color: var(--clr-bg-1-muted);
		border: 1px solid transparent;
		border-bottom: none;
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		transition: color var(--transition-fast);

		&.active {
			color: var(--clr-text-1);
			background-color: var(--clr-bg-1);
			border-color: var(--clr-border-2);

			&:after {
				content: '';
				position: absolute;
				bottom: 0;
				left: 0;
				width: 100%;
				height: 1px;
				background-color: var(--clr-border-3);
				transform: translateY(100%);
			}
		}

		&:hover {
			color: var(--clr-text-1);
		}
	}

	.message-editor-wrapper {
		flex: 1;
		border-radius: 0 var(--radius-m) var(--radius-m) var(--radius-m);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;

		&:hover,
		&:focus-within {
			border-color: var(--clr-border-1);
		}
	}
</style>
