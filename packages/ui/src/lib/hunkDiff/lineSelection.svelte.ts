import type { Row } from '$lib/utils/diffParsing';

export interface LineSelectionParams {
	index: number;
	oldLine: number | undefined;
	newLine: number | undefined;
	shift: boolean;
	ctrlOrMeta: boolean;
	startIndex: number;
	rows: Row[] | undefined;
}

type ToggleLineSelectionFn = (params: LineSelectionParams) => void;

export default class LineSelection {
	private rows: Row[] | undefined;
	private _selectionStart = $state<number>();

	constructor(private onLineClick: ToggleLineSelectionFn | undefined) {}

	setRows(rows: Row[]) {
		this.rows = rows;
	}

	onStart(ev: MouseEvent, row: Row, index: number) {
		ev.preventDefault();
		ev.stopPropagation();

		this._selectionStart = index;
		this.onLineClick?.({
			index,
			oldLine: row.beforeLineNumber,
			newLine: row.afterLineNumber,
			shift: ev.shiftKey,
			ctrlOrMeta: ev.ctrlKey || ev.metaKey,
			startIndex: index,
			rows: this.rows
		});
	}

	onMoveOver(ev: MouseEvent, row: Row, index: number) {
		if (this._selectionStart === undefined) return;
		if (ev.buttons === 1) {
			ev.preventDefault();
			ev.stopPropagation();

			this.onLineClick?.({
				index,
				oldLine: row.beforeLineNumber,
				newLine: row.afterLineNumber,
				shift: ev.shiftKey,
				ctrlOrMeta: ev.ctrlKey || ev.metaKey,
				startIndex: this._selectionStart,
				rows: this.rows
			});
		}
	}

	onEnd() {
		this._selectionStart = undefined;
	}
}
