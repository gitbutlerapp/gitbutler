<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { EditorState } from '@codemirror/state';
	import { EditorView, lineNumbers } from '@codemirror/view';

	let editorTheme = EditorView.theme(
		{
			'&': {
				color: '#d4d4d8',
				backgroundColor: '#18181b'
			},
			'.cm-content': {
				caretColor: '#0e9'
			},
			'&.cm-focused .cm-cursor': {
				borderLeftColor: '#0e9'
			},
			'&.cm-focused .cm-selectionBackground, ::selection': {
				backgroundColor: '#0284c7'
			},
			'.cm-gutters': {
				backgroundColor: '#18181b',
				color: '#3f3f46',
				border: 'none'
			}
		},
		{ dark: true }
	);
	const fixedHeightEditor = EditorView.theme({
		'&': { height: '600px' },
		'.cm-scroller': { overflow: 'auto' }
	});

	export let value: string;

	let element: HTMLDivElement;
	let editorView: EditorView;

	onMount(() => (editorView = create_editor_view(value)));
	onDestroy(() => editorView?.destroy());

	$: editorView && update(value);

	// There may be a more graceful way to update the two editors
	function update(value: string): void {
		editorView?.destroy();
		editorView = create_editor_view(value);
	}

	function create_editor_state(value: string | null | undefined): EditorState {
		return EditorState.create({
			doc: value ?? undefined,
			extensions: state_extensions
		});
	}

	function create_editor_view(value: string): EditorView {
		return new EditorView({
			state: create_editor_state(value),
			parent: element
		});
	}

	let state_extensions = [
		EditorView.editable.of(false),
		EditorView.lineWrapping,
		lineNumbers(),
		editorTheme,
		fixedHeightEditor
	];
</script>

<code>
	<div bind:this={element} />
</code>
