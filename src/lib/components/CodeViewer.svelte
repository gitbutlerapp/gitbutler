<script lang="ts">
	import { Operation, type Delta } from '$lib/deltas';
	import { EditorState, type ChangeSpec, type TransactionSpec } from '@codemirror/state';
	import { EditorView, lineNumbers } from '@codemirror/view';

	export let doc: string;
	export let deltas: Delta[];
	export let end: number;

	const editorTheme = EditorView.theme(
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

	const convertOperation = (operation: Operation): ChangeSpec => {
		if (Operation.isInsert(operation)) {
			return {
				from: operation.insert[0],
				insert: operation.insert[1]
			};
		} else if (Operation.isDelete(operation)) {
			return {
				from: operation.delete[0],
				to: operation.delete[0] + operation.delete[1]
			};
		} else {
			throw new Error(`${operation} is not supported`);
		}
	};

	const extensions = [
		EditorView.editable.of(false),
		EditorView.lineWrapping,
		lineNumbers(),
		editorTheme,
		fixedHeightEditor
	];

	type EditorParams = { doc: string; deltas: Delta[]; end: number };

	const deriveState = (doc: string, deltas: Delta[]) =>
		deltas
			.flatMap((delta) => delta.operations)
			.map(convertOperation)
			.map(
				(change): TransactionSpec => ({
					changes: [change],
					sequential: true,
					scrollIntoView: true
				})
			)
			.reduce((state, transactionSpec) => {
				const tx = state.update(transactionSpec);
				return tx.state;
			}, EditorState.create({ doc, extensions }));

	const editor = (parent: HTMLElement, { doc, deltas, end }: EditorParams) => {
		deltas = deltas.filter(({ timestampMs }) => timestampMs <= end);
		const state = deriveState(doc, deltas);
		const view = new EditorView({ state, parent });
		return {
			update: ({ doc, deltas, end }: EditorParams) => {
				deltas = deltas.filter(({ timestampMs }) => timestampMs <= end);
				const state = deriveState(doc, deltas);
				view.setState(state);
			},
			destroy: () => view.destroy()
		};
	};
</script>

<code use:editor={{ doc, deltas, end }} />
