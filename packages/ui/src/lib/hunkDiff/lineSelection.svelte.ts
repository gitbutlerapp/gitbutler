import { isTouchDevice } from '$lib/utils/browserAgent';
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

interface TouchCoords {
	x: number;
	y: number;
}

export default class LineSelection {
	private readonly touchDevice = isTouchDevice();
	private rows: Row[] | undefined;
	private _touchStart = $state<TouchCoords>();
	private _touchMove = $state<TouchCoords>();
	private _selectionStart = $state<number>();
	private _selectionEnd = $state<number>();

	constructor(private onLineClick: ToggleLineSelectionFn | undefined) {}

	setRows(rows: Row[]) {
		this.rows = rows;
	}

	onStart(ev: MouseEvent, row: Row, index: number) {
		if (this.touchDevice) return;
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
		if (this.touchDevice) return;
		if (this._selectionStart === undefined) return;
		if (ev.buttons === 1) {
			ev.preventDefault();
			ev.stopPropagation();

			this._selectionEnd = index;
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
		this._touchMove = undefined;
		this._touchStart = undefined;
		this._selectionStart = undefined;
		this._selectionEnd = undefined;
	}

	onTouchStart(ev: TouchEvent) {
		this._touchStart = { x: ev.touches[0].clientX, y: ev.touches[0].clientY };
	}

	onTouchMove(ev: TouchEvent) {
		this._touchMove = { x: ev.touches[0].clientX, y: ev.touches[0].clientY };
	}

	get touchStart() {
		return this._touchStart;
	}

	get touchMove() {
		return this._touchMove;
	}

	touchSelectionStart(row: Row, index: number) {
		if (this._selectionStart !== undefined) return;
		this._selectionStart = index;
		this.onLineClick?.({
			index,
			oldLine: row.beforeLineNumber,
			newLine: row.afterLineNumber,
			shift: false,
			ctrlOrMeta: false,
			startIndex: index,
			rows: this.rows
		});
	}

	touchSelectionEnd(row: Row, index: number) {
		if (this._selectionStart === undefined || this._selectionEnd === index) return;
		this._selectionEnd = index;
		this.onLineClick?.({
			index,
			oldLine: row.beforeLineNumber,
			newLine: row.afterLineNumber,
			shift: false,
			ctrlOrMeta: false,
			startIndex: this._selectionStart,
			rows: this.rows
		});
	}
}
