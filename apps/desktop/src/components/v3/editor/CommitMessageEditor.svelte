<script lang="ts">
	import { showError } from '$lib/notifications/toasts';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import GiphyPlugin from '@gitbutler/ui/richText/plugins/GiphyPlugin.svelte';
	import FormattingPopup from '@gitbutler/ui/richText/tools/FormattingPopup.svelte';

	interface Props {
		markdown: boolean;
	}

	const { markdown = $bindable() }: Props = $props();

	let composer = $state<ReturnType<typeof RichTextEditor>>();

	export async function getPlaintext(): Promise<string | undefined> {
		return composer?.getPlaintext();
	}
</script>

<RichTextEditor
	styleContext="client-editor"
	namespace="CommitMessageEditor"
	placeholder="Your commit message"
	bind:this={composer}
	{markdown}
	onError={(e) => showError('Editor error', e)}
>
	{#snippet toolBar()}
		<FormattingPopup />
	{/snippet}
	{#snippet plugins()}
		<GiphyPlugin />
	{/snippet}
</RichTextEditor>
