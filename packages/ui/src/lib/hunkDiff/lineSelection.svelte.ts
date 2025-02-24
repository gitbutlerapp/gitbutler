import type { Row } from '$lib/utils/diffParsing';

type SelectionDirection = 'up' | 'down';

export interface LineSelectionParams {
	index: number;
	oldLine: number | undefined;
	newLine: number | undefined;
	shift: boolean;
	ctrlOrMeta: boolean;
	resetSelection?: boolean;
}

type ToggleLineSelectionFn = (params: LineSelectionParams) => void;

export default class LineSelection {
	private rows: Row[] | undefined;
	private _selectionStart = $state<number>();
	private _lastSelected = $state<number>();
	private _selectionDirection: SelectionDirection | undefined = $derived.by(() => {
		if (this._selectionStart === undefined || this._lastSelected === undefined) return;
		if (this._selectionStart === this._lastSelected) return;
		return this._selectionStart < this._lastSelected ? 'down' : 'up';
	});

	constructor(private onLineClick: ToggleLineSelectionFn | undefined) {}

	setRows(rows: Row[]) {
		this.rows = rows;
	}

	onStart(ev: MouseEvent, row: Row, index: number) {
		ev.preventDefault();
		ev.stopPropagation();
		this._selectionStart = index;
		this._lastSelected = index;
		this.onLineClick?.({
			index,
			oldLine: row.beforeLineNumber,
			newLine: row.afterLineNumber,
			shift: ev.shiftKey,
			ctrlOrMeta: ev.ctrlKey || ev.metaKey,
			resetSelection: true
		});
	}

	/**
	 * Determine whether the selection direction has been reversed,
	 * based on the current index, the last selected index, and the
	 * initial selection direction.
	 */
	private isReversedDragSelection(
		index: number,
		lastSelected: number,
		selectionDirection: SelectionDirection
	) {
		if (selectionDirection === 'up') {
			return index > lastSelected;
		}

		if (selectionDirection === 'down') {
			return index < lastSelected;
		}

		return false;
	}

	onMoveOver(ev: MouseEvent, row: Row, index: number) {
		if (this._lastSelected === index) return;
		if (ev.buttons === 1) {
			ev.preventDefault();
			ev.stopPropagation();
			toggleLine: {
				if (this._lastSelected !== undefined && this._selectionDirection !== undefined) {
					if (this.isReversedDragSelection(index, this._lastSelected, this._selectionDirection)) {
						// Handle the case in which the user drag selects in one direction,
						// and then it reverses the direction of the selection in order to
						// unselect the last item
						const lastRow = this.rows?.[this._lastSelected];
						if (!lastRow) break toggleLine;

						this.onLineClick?.({
							index: this._lastSelected,
							oldLine: lastRow.beforeLineNumber,
							newLine: lastRow.afterLineNumber,
							shift: ev.shiftKey,
							ctrlOrMeta: ev.ctrlKey || ev.metaKey
						});

						break toggleLine;
					}
				}

				this.onLineClick?.({
					index,
					oldLine: row.beforeLineNumber,
					newLine: row.afterLineNumber,
					shift: ev.shiftKey,
					ctrlOrMeta: ev.ctrlKey || ev.metaKey
				});
			}
			this._lastSelected = index;
		}
	}

	onEnd() {
		this._selectionStart = undefined;
		this._lastSelected = undefined;
	}
}
