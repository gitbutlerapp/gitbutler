import { EditorView, Decoration, type DecorationSet } from '@codemirror/view';
import { StateField, StateEffect, EditorSelection, RangeSet } from '@codemirror/state';

const addMark = StateEffect.define<{ from: number; to: number }>({
	map: ({ from, to }, change) => ({ from: change.mapPos(from), to: change.mapPos(to) })
});

const mark = Decoration.mark({ class: 'cm-mark' });

const changes = StateField.define<DecorationSet>({
	create: () => Decoration.none,
	update: (_old, transaction) =>
		RangeSet.of(
			transaction.effects
				.filter((effect) => effect.is(addMark))
				.map((effect) => mark.range(effect.value.from, effect.value.to))
		),
	provide: (field) => EditorView.decorations.from(field)
});

export const markChanges = (selection: EditorSelection | undefined) => {
	if (selection === undefined) return undefined;

	const effects: StateEffect<{ from: number; to: number }>[] = selection.ranges
		.filter((r) => !r.empty)
		.map(({ from, to }) => addMark.of({ from, to }));

	return effects.length > 0 ? [...effects, StateEffect.appendConfig.of([changes])] : undefined;
};
