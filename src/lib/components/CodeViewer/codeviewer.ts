import { Operation, type Delta } from '$lib/deltas';
import { EditorState, type ChangeSpec, type TransactionSpec } from '@codemirror/state';
import { EditorView, lineNumbers } from '@codemirror/view';
import { colorEditor, highLightSyntax } from './colors';
import { getLanguage } from './languages';

const sizes = EditorView.theme({
	'&': { height: '100%', width: '100%' },
	'.cm-scroller': { overflow: 'scroll' }
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

const applyDeltas = (state: EditorState, ...deltas: Delta[]): EditorState =>
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
		}, state);

const extensions = [
	colorEditor,
	highLightSyntax,
	EditorView.editable.of(false),
	EditorView.lineWrapping,
	lineNumbers(),
	sizes
];

type Params = { doc: string; deltas: Delta[]; end: number; filepath: string };

type State = {
	filepath: string;
	base: EditorState;
	deltas: Delta[];
	// states has the same length as deltas and each state is the result of applying the deltas to base up to that index
	states: EditorState[];
};

const makeBaseState = (doc: string, filepath: string) => {
	const language = getLanguage(filepath);
	return EditorState.create({
		doc,
		extensions: language ? [...extensions, language] : extensions
	});
};

// finds last deltas where timestampMs <= ts, or returns the last delta index
const deltaIndex = (deltas: Delta[], ts: number) => {
	let i = 0;
	while (i < deltas.length && deltas[i].timestampMs <= ts) {
		i++;
	}
	return i - 1;
};

const initComponentState = (params: Params): State => {
	const { doc, deltas, filepath } = params;
	const base = makeBaseState(doc, filepath);
	return {
		filepath,
		base,
		deltas,
		states: deltas.reduce(
			(states, delta) => [...states, applyDeltas(states[states.length - 1], delta)],
			[base]
		)
	};
};

const findState = (componentState: State, ts: number) => {
	const index = deltaIndex(componentState.deltas, ts);
	return index === -1
		? componentState.states[componentState.states.length - 1]
		: componentState.states[index];
};

export default (parent: HTMLElement, { doc, deltas, end, filepath }: Params) => {
	let componentState = initComponentState({ doc, deltas, end, filepath });
	const state = findState(componentState, end);
	const view = new EditorView({ state, parent });
	return {
		update: ({ end, filepath, deltas, doc }: Params) => {
			if (filepath !== componentState.filepath) {
				componentState = initComponentState({ doc, deltas, end, filepath });
			}
			const state = findState(componentState, end);
			view.setState(state);
		},
		destroy: () => view.destroy()
	};
};
