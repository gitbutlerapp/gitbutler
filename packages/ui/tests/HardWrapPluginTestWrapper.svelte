<script lang="ts">
	import RichTextEditor from '$lib/richText/RichTextEditor.svelte';
	import HardWrapPlugin from '$lib/richText/plugins/HardWrapPlugin.svelte';

	type Props = {
		maxLength?: number;
		enabled?: boolean;
		initialText?: string;
	};

	const { maxLength = 50, enabled = true, initialText = '' }: Props = $props();

	let editor = $state<RichTextEditor>();
	let paragraphCountDisplay = $state(0);
	let textContentDisplay = $state('');
	let value = $state(initialText);

	// Update display values from the editor
	async function updateDisplayValues() {
		if (!editor) return;
		textContentDisplay = (await editor.getPlaintext()) || '';
		paragraphCountDisplay = await editor.getParagraphCount();
	}

	// Poll for updates (since we don't have direct access to editor state changes)
	$effect(() => {
		if (editor) {
			const interval = setInterval(updateDisplayValues, 100);
			return () => clearInterval(interval);
		}
	});

	function setText(text: string) {
		if (!editor) return;
		editor.setText(text);
		setTimeout(updateDisplayValues, 100);
	}

	function wrapAll() {
		if (!editor) return;
		editor.wrapAll();
		setTimeout(updateDisplayValues, 200);
	}

	function focusEditor() {
		if (!editor) return;
		editor.focus();
	}

	function clear() {
		if (!editor) return;
		editor.clear();
		setTimeout(updateDisplayValues, 100);
	}
</script>

<div class="test-container" data-testid="test-container">
	<div class="editor-wrapper" data-testid="editor-wrapper">
		<RichTextEditor
			bind:this={editor}
			bind:value
			namespace="test-hardwrap"
			plaintext={true}
			onError={(error) => console.error('Editor error:', error)}
			styleContext="client-editor"
			wrapCountValue={maxLength}
			{initialText}
			minHeight="100px"
		>
			{#snippet plugins()}
				<HardWrapPlugin {maxLength} {enabled} />
			{/snippet}
		</RichTextEditor>
	</div>

	<div class="controls">
		<button type="button" data-testid="set-text-button" onclick={() => setText('Test text')}>
			Set Text
		</button>
		<button type="button" data-testid="wrap-all-button" onclick={wrapAll}>Wrap All</button>
		<button type="button" data-testid="focus-button" onclick={focusEditor}>Focus</button>
		<button type="button" data-testid="clear-button" onclick={clear}>Clear</button>
	</div>

	<div class="debug-info">
		<span data-testid="paragraph-count">{paragraphCountDisplay}</span>
		<span data-testid="text-content">{textContentDisplay}</span>
	</div>
</div>

<style>
	.test-container {
		width: 600px;
		min-height: 200px;
		padding: 16px;
		border: 1px solid #ccc;
		background: white;
	}

	.editor-wrapper {
		min-height: 100px;
		margin-bottom: 8px;
		border: 1px solid #eee;
	}

	.controls {
		display: flex;
		margin-bottom: 8px;
		gap: 8px;
	}

	button {
		padding: 4px 8px;
		font-size: 12px;
	}

	.debug-info {
		display: none;
	}
</style>
