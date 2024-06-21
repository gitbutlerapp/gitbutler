export function getSelectionDirection(firstFileIndex: number, lastFileIndex: number) {
	// detect the direction of the selection
	const selectionDirection = lastFileIndex < firstFileIndex ? 'down' : 'up';

	return selectionDirection;
}
