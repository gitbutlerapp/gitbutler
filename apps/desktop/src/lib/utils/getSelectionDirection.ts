export function getSelectionDirection(
	firstFileIndex: number,
	lastFileIndex: number
): 'up' | 'down' {
	// detect the direction of the selection
	const selectionDirection = lastFileIndex < firstFileIndex ? 'down' : 'up';

	return selectionDirection;
}
