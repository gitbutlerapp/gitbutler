<script lang="ts">
	import RichTextEditor from '$lib/richText/RichTextEditor.svelte';
	import IndentPlugin from '$lib/richText/plugins/IndentPlugin.svelte';

	type Props = {
		initialText?: string;
	};

	const { initialText = '' }: Props = $props();

	let editor = $state<RichTextEditor>();
	let paragraphCountDisplay = $state(0);
	let textContentDisplay = $state('');
	let value = $state(initialText);

	// Update display values from the editor
	async function updateDisplayValues() {
		if (!editor) return;
		const plaintext = await editor.getPlaintext();
		const count = await editor.getParagraphCount();
		textContentDisplay = plaintext || '';
		paragraphCountDisplay = count || 0;
	}

	// Poll for updates
	$effect(() => {
		if (editor) {
			// Initial update
			updateDisplayValues();
			const interval = setInterval(updateDisplayValues, 100);
			return () => clearInterval(interval);
		}
	});

	function focusEditor() {
		if (!editor) return;
		editor.focus();
	}
</script>

<div class="test-container" data-testid="test-container">
	<div class="editor-wrapper" data-testid="editor-wrapper">
		<RichTextEditor
			bind:this={editor}
			bind:value
			namespace="test-plaintext-indent"
			onError={(error) => console.error('Editor error:', error)}
			styleContext="client-editor"
			{initialText}
			minHeight="100px"
		>
			{#snippet plugins()}
				<IndentPlugin />
			{/snippet}
		</RichTextEditor>
	</div>

	<div class="controls">
		<button type="button" data-testid="focus-button" onclick={focusEditor}>Focus</button>
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
