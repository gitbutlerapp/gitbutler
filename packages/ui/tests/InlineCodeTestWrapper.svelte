<script lang="ts">
	import RichTextEditor from "$lib/richText/RichTextEditor.svelte";

	type Props = {
		initialText?: string;
	};

	const { initialText = "" }: Props = $props();

	let editor = $state<RichTextEditor>();
	let textContentDisplay = $state("");
	let inlineCodeCountDisplay = $state(0);
	let value = $state(initialText);

	async function updateDisplayValues() {
		if (!editor) return;
		textContentDisplay = (await editor.getPlaintext()) || "";
		inlineCodeCountDisplay = await getInlineCodeCount();
	}

	function getInlineCodeCount(): Promise<number> {
		return new Promise((resolve) => {
			if (!editor) {
				resolve(0);
				return;
			}
			// Access the editor internals through getPlaintext's pattern
			// We need to read from the editor state
			editor.getPlaintext().then(() => {
				// The editor is available, we can use save() to inspect state
				const state = editor!.save();
				if (!state) {
					resolve(0);
					return;
				}
				let count = 0;
				function walk(node: any) {
					if (node.type === "inline-code") {
						count++;
					}
					if (node.children) {
						for (const child of node.children) {
							walk(child);
						}
					}
				}
				walk(state.root);
				resolve(count);
			});
		});
	}

	$effect(() => {
		if (editor) {
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
			namespace="test-inline-code"
			onError={(error) => console.error("Editor error:", error)}
			styleContext="client-editor"
			{initialText}
			minHeight="100px"
		/>
	</div>

	<div class="controls">
		<button type="button" data-testid="focus-button" onclick={focusEditor}>Focus</button>
	</div>

	<div class="debug-info">
		<span data-testid="inline-code-count">{inlineCodeCountDisplay}</span>
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
