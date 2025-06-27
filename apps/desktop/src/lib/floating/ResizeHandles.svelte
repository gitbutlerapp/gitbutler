<script lang="ts">
	interface Props {
		onResizeStart: (event: PointerEvent, direction: string) => void;
		snapPosition: string;
	}

	const { onResizeStart, snapPosition }: Props = $props();

	function getCursorStyle(direction: string): string {
		const cursors: Record<string, string> = {
			n: 'ns-resize',
			s: 'ns-resize',
			e: 'ew-resize',
			w: 'ew-resize',
			ne: 'nesw-resize',
			sw: 'nesw-resize',
			nw: 'nwse-resize',
			se: 'nwse-resize'
		};
		return cursors[direction] || 'default';
	}

	function isResizeAllowed(direction: string): boolean {
		// Don't allow resizing from snapped edges
		if (snapPosition.includes('left') && direction.includes('w')) return false;
		if (snapPosition.includes('right') && direction.includes('e')) return false;
		if (snapPosition.includes('top') && direction.includes('n')) return false;
		if (snapPosition.includes('bottom') && direction.includes('s')) return false;

		// For center positions, allow all resizing
		return true;
	}

	function handleResizeStart(event: PointerEvent, direction: string) {
		if (!isResizeAllowed(direction)) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		onResizeStart(event, direction);
	}
</script>

<!-- Corner handles -->
<div
	class="resize-handle corner nw"
	class:disabled={!isResizeAllowed('nw')}
	style="cursor: {isResizeAllowed('nw') ? getCursorStyle('nw') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'nw')}
></div>
<div
	class="resize-handle corner ne"
	class:disabled={!isResizeAllowed('ne')}
	style="cursor: {isResizeAllowed('ne') ? getCursorStyle('ne') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'ne')}
></div>
<div
	class="resize-handle corner sw"
	class:disabled={!isResizeAllowed('sw')}
	style="cursor: {isResizeAllowed('sw') ? getCursorStyle('sw') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'sw')}
></div>
<div
	class="resize-handle corner se"
	class:disabled={!isResizeAllowed('se')}
	style="cursor: {isResizeAllowed('se') ? getCursorStyle('se') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'se')}
></div>

<!-- Edge handles -->
<div
	class="resize-handle edge n"
	class:disabled={!isResizeAllowed('n')}
	style="cursor: {isResizeAllowed('n') ? getCursorStyle('n') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'n')}
></div>
<div
	class="resize-handle edge s"
	class:disabled={!isResizeAllowed('s')}
	style="cursor: {isResizeAllowed('s') ? getCursorStyle('s') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 's')}
></div>
<div
	class="resize-handle edge w"
	class:disabled={!isResizeAllowed('w')}
	style="cursor: {isResizeAllowed('w') ? getCursorStyle('w') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'w')}
></div>
<div
	class="resize-handle edge e"
	class:disabled={!isResizeAllowed('e')}
	style="cursor: {isResizeAllowed('e') ? getCursorStyle('e') : 'not-allowed'}"
	onpointerdown={(e) => handleResizeStart(e, 'e')}
></div>

<style>
	.resize-handle {
		z-index: 10;
		position: absolute;
	}

	.resize-handle.corner {
		width: 12px;
		height: 12px;
	}

	.resize-handle.nw {
		top: 0;
		left: 0;
	}
	.resize-handle.ne {
		top: 0;
		right: 0;
	}
	.resize-handle.sw {
		bottom: 0;
		left: 0;
	}
	.resize-handle.se {
		right: 0;
		bottom: 0;
	}

	.resize-handle.edge.n {
		top: 0;
		right: 12px;
		left: 12px;
		height: 8px;
	}
	.resize-handle.edge.s {
		right: 12px;
		bottom: 0;
		left: 12px;
		height: 8px;
	}
	.resize-handle.edge.w {
		top: 12px;
		bottom: 12px;
		left: 0;
		width: 8px;
	}
	.resize-handle.edge.e {
		top: 12px;
		right: 0;
		bottom: 12px;
		width: 8px;
	}

	.resize-handle.disabled {
		cursor: not-allowed !important;
		opacity: 0.3;
		pointer-events: none !important;
	}
</style>
