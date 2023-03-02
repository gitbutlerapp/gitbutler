import { Operation, type Delta } from '$lib/deltas';
import {
    Text,
    ChangeSet,
    EditorState,
    EditorSelection,
    type ChangeSpec,
    SelectionRange
} from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { getLanguage } from './languages';
import extensions from './extensions';
import { markChanges } from './mark';

const toChangeSpec = (operation: Operation): ChangeSpec => {
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

type Params = { doc: string; deltas: Delta[]; filepath: string };

const makeBaseState = (doc: string, filepath: string) => {
    const language = getLanguage(filepath);
    return EditorState.create({
        doc,
        extensions: language ? [...extensions, language] : extensions
    });
};

const toChangeSet = (deltas: Delta[], initLength: number): ChangeSet => {
    const specs = deltas.flatMap(({ operations }) => operations.map(toChangeSpec));
    const sets = specs.reduce((sets: ChangeSet[], spec) => {
        const set = ChangeSet.of(spec, sets.length > 0 ? sets[sets.length - 1].newLength : initLength);
        return [...sets, set];
    }, [] as ChangeSet[]);
    return sets.length > 0 ? sets.reduce((a, b) => a.compose(b)) : ChangeSet.empty(initLength);
};

const toRange = (operation: Operation) => {
    if (Operation.isInsert(operation)) {
        const anchor = operation.insert[0];
        const head = operation.insert[0] + operation.insert[1].length;
        return EditorSelection.range(anchor, head);
    } else if (Operation.isDelete(operation)) {
        const anchor = operation.delete[0];
        const head = operation.delete[0] + operation.delete[1];
        return EditorSelection.range(anchor, head);
    } else {
        return undefined;
    }
};

const toSelection = (changes: ChangeSet, delta: Delta | undefined): EditorSelection | undefined => {
    if (delta === undefined) return undefined;
    if (delta.operations.length === 0) return undefined;
    const ranges = delta.operations
        .map(toRange)
        .filter((r): r is SelectionRange => r !== undefined)
        .filter((range) => range.head <= changes.newLength && range.anchor <= changes.newLength);
    return ranges.length ? EditorSelection.create(ranges) : undefined;
};

// this action assumes:
// * that deltas list is append only.
// * that each (filepath, doc) pair never changes.
export default (parent: HTMLElement, { doc, deltas, filepath }: Params) => {
    const view = new EditorView({ state: makeBaseState(doc, filepath), parent });

    view.dispatch(
        view.state.update({
            changes: toChangeSet(deltas, doc.length)
        })
    );

    let currentFilepath = filepath;
    const stateCache: Record<string, EditorState> = {};
    const deltasCache: Record<string, Delta[]> = {};

    stateCache[filepath] = view.state;
    deltasCache[filepath] = deltas;

    return {
        update: ({ doc, deltas: newDeltas, filepath }: Params) => {
            if (filepath !== currentFilepath) {
                view.setState(stateCache[filepath] ?? makeBaseState(doc, filepath));
            }

            const currentDeltas = deltasCache[filepath] || [];
            if (currentDeltas.length > newDeltas.length) {
                // rewind backward

                const baseText = Text.of([doc]);
                const targetChange = toChangeSet(newDeltas, baseText.length);
                const targetText = targetChange.apply(baseText);

                const deltasToRevert = currentDeltas.slice(newDeltas.length);
                const revertChange = toChangeSet(deltasToRevert, targetText.length);
                const changes = revertChange.invert(targetText);

                const selection = toSelection(changes, newDeltas.at(-1));

                view.dispatch({
                    changes,
                    selection,
                    scrollIntoView: true,
                    effects: markChanges(selection)
                });
            } else {
                // rewind forward

                // verify that deltas are append only
                currentDeltas.forEach((delta, i) => {
                    if (i >= newDeltas.length) return;
                    if (delta !== newDeltas[i]) throw new Error('deltas are not append only');
                });

                const deltasToApply = newDeltas.slice(currentDeltas.length);
                const changes = toChangeSet(deltasToApply, view.state.doc.length);
                const selection = toSelection(changes, deltasToApply.at(-1));

                view.dispatch({
                    changes,
                    selection,
                    scrollIntoView: true,
                    effects: markChanges(selection)
                });
            }

            // don't forget to update caches
            stateCache[filepath] = view.state;
            deltasCache[filepath] = newDeltas;
            currentFilepath = filepath;
        },
        destroy: () => view.destroy()
    };
};
