// floatingDiv.ts - Svelte 5 action for making elements draggable and resizable
// Usage: <div use:floatingDiv={{ handleSelector: '.drag-handle', resizable: true }}>...</div>

/**
 * Interface for position coordinates
 */
interface Position {
	x: number;
	y: number;
}

/**
 * Interface for element size dimensions
 */
interface Size {
	width: number;
	height: number;
}

/**
 * Interface for resize handle configuration
 */
interface ResizeHandles {
	right?: boolean;
	bottom?: boolean;
	corner?: boolean;
}

/**
 * Interface for resize handle elements
 */
interface ResizeHandleElements {
	right: HTMLDivElement | null;
	bottom: HTMLDivElement | null;
	corner: HTMLDivElement | null;
}

/**
 * Interface for drag event data
 */
interface DragEventData {
	node: HTMLElement;
	position: Position;
}

/**
 * Interface for resize event data
 */
interface ResizeEventData {
	node: HTMLElement;
	size: Size;
	mode: ResizeMode | null;
}

/**
 * Type for resize mode
 */
type ResizeMode = 'right' | 'bottom' | 'corner';

/**
 * Interface for floating div action options
 */
export interface FloatingDivOptions {
	initialPosition: { x: number; y: number };
	initialSize: { width: number; height: number };
	resizeHandles?: ResizeHandles;
	handleSelector?: string | null;
	zIndex?: number;
	minWidth?: number;
	minHeight?: number;
	maxWidth?: number | null;
	maxHeight?: number | null;
	onDragStart?: ((data: DragEventData) => void) | null;
	onDragEnd?: ((data: DragEventData) => void) | null;
	onResizeStart?: ((data: ResizeEventData) => void) | null;
	onResizeEnd?: ((data: ResizeEventData) => void) | null;
}

/**
 * Interface for Svelte action return type
 */
interface ActionReturn {
	update: (options: FloatingDivOptions) => void;
	destroy: () => void;
}

/**
 * Creates a draggable and resizable floating div
 * @param {HTMLElement} node - The element to make draggable and resizable
 * @param {FloatingDivOptions} options - Configuration options
 * @returns {ActionReturn} Svelte action return object
 */
export function floatingDiv(node: HTMLElement, options: FloatingDivOptions): ActionReturn {
	const {
		handleSelector = null,
		onDragStart = null,
		onDragEnd = null,
		resizeHandles = { right: true, bottom: true, corner: true },
		minWidth = 100,
		minHeight = 100,
		maxWidth = null,
		maxHeight = null,
		onResizeStart = null,
		onResizeEnd = null
	} = options;
	let { initialPosition, initialSize, zIndex = 100 } = options;

	let dragHandle: HTMLElement | null = handleSelector ? node.querySelector(handleSelector) : node;
	let currentPosition: Position = { x: 0, y: 0 };
	let offset: Position = { x: 0, y: 0 };
	let isDragging: boolean = false;

	// Resize state
	let isResizing: boolean = false;
	let resizeMode: ResizeMode | null = null;
	let resizeHandleElements: ResizeHandleElements = {
		right: null,
		bottom: null,
		corner: null
	};

	// Initialize position (use current position if already set)
	const nodeStyle = getComputedStyle(node);
	const nodeLeft = initialPosition.x || 0;
	const nodeTop = initialPosition.y || 0;

	// Set initial styles
	node.style.position = 'absolute';
	node.style.left = `${nodeLeft}px`;
	node.style.top = `${nodeTop}px`;

	// Set initial size
	const initialWidth = initialSize.width || 200;
	const initialHeight = initialSize.height || 200;
	node.style.width = `${initialWidth}px`;
	node.style.height = `${initialHeight}px`;

	// Store initial z-index to restore later
	const initialZIndex = nodeStyle.zIndex;

	// Create resize handles if resizable is enabled
	function createResizeHandles(): void {
		if (!resizeHandles) return;

		// Clean up any existing resize handles first
		removeResizeHandles();

		// Create new resize handles
		if (resizeHandles.right) {
			const rightHandle = document.createElement('div');
			rightHandle.className = 'resize-handle resize-handle-right';
			rightHandle.style.cssText = `
        position: absolute;
        right: -4px;
        top: 0;
        width: 8px;
        height: 100%;
        cursor: ew-resize;
        z-index: ${zIndex + 1};
      `;
			node.appendChild(rightHandle);
			resizeHandleElements.right = rightHandle;

			rightHandle.addEventListener('mousedown', (e) => handleResizeStart(e, 'right'));
			rightHandle.addEventListener('touchstart', (e) => handleResizeTouchStart(e, 'right'));
		}

		if (resizeHandles.bottom) {
			const bottomHandle = document.createElement('div');
			bottomHandle.className = 'resize-handle resize-handle-bottom';
			bottomHandle.style.cssText = `
        position: absolute;
        bottom: -4px;
        left: 0;
        width: 100%;
        height: 8px;
        cursor: ns-resize;
        z-index: ${zIndex + 1};
      `;
			node.appendChild(bottomHandle);
			resizeHandleElements.bottom = bottomHandle;

			bottomHandle.addEventListener('mousedown', (e) => handleResizeStart(e, 'bottom'));
			bottomHandle.addEventListener('touchstart', (e) => handleResizeTouchStart(e, 'bottom'));
		}

		if (resizeHandles.corner) {
			const cornerHandle = document.createElement('div');
			cornerHandle.className = 'resize-handle resize-handle-corner';
			cornerHandle.style.cssText = `
        position: absolute;
        bottom: -4px;
        right: -4px;
        width: 12px;
        height: 12px;
        cursor: nwse-resize;
        z-index: ${zIndex + 2};
      `;
			node.appendChild(cornerHandle);
			resizeHandleElements.corner = cornerHandle;

			cornerHandle.addEventListener('mousedown', (e) => handleResizeStart(e, 'corner'));
			cornerHandle.addEventListener('touchstart', (e) => handleResizeTouchStart(e, 'corner'));
		}
	}

	function removeResizeHandles(): void {
		Object.values(resizeHandleElements).forEach((handle) => {
			if (handle && handle.parentNode) {
				handle.parentNode.removeChild(handle);
			}
		});

		resizeHandleElements = {
			right: null,
			bottom: null,
			corner: null
		};
	}

	function handleResizeStart(event: MouseEvent, mode: ResizeMode): void {
		// Only process left mouse button
		if (event.button !== 0) return;

		event.preventDefault();
		event.stopPropagation(); // Prevent dragging from starting

		isResizing = true;
		resizeMode = mode;

		initialPosition = { x: event.clientX, y: event.clientY };
		initialSize = {
			width: parseInt(node.style.width, 10) || node.offsetWidth,
			height: parseInt(node.style.height, 10) || node.offsetHeight
		};

		// Apply resizing styles
		node.style.zIndex = String(zIndex);

		// Add event listeners for resizing
		window.addEventListener('mousemove', handleResizeMove);
		window.addEventListener('mouseup', handleResizeEnd);

		if (onResizeStart)
			onResizeStart({
				node,
				size: { ...initialSize },
				mode: resizeMode
			});
	}

	function handleResizeTouchStart(event: TouchEvent, mode: ResizeMode): void {
		if (event.touches.length !== 1) return;

		const touch = event.touches[0]!;
		const mouseEvent = new MouseEvent('mousedown', {
			clientX: touch.clientX,
			clientY: touch.clientY,
			button: 0
		});

		handleResizeStart(mouseEvent, mode);
	}

	function handleResizeMove(event: MouseEvent): void {
		if (!isResizing) return;

		event.preventDefault();

		const deltaX = event.clientX - initialPosition.x;
		const deltaY = event.clientY - initialPosition.y;

		let newWidth = initialSize.width;
		let newHeight = initialSize.height;

		// Apply size changes based on resize mode
		if (resizeMode === 'right' || resizeMode === 'corner') {
			newWidth = Math.max(minWidth, initialSize.width + deltaX);
			if (maxWidth !== null) newWidth = Math.min(maxWidth, newWidth);
		}

		if (resizeMode === 'bottom' || resizeMode === 'corner') {
			newHeight = Math.max(minHeight, initialSize.height + deltaY);
			if (maxHeight !== null) newHeight = Math.min(maxHeight, newHeight);
		}

		// Update size
		node.style.width = `${newWidth}px`;
		node.style.height = `${newHeight}px`;
	}

	function handleResizeEnd(): void {
		if (!isResizing) return;

		isResizing = false;

		// Remove event listeners
		window.removeEventListener('mousemove', handleResizeMove);
		window.removeEventListener('mouseup', handleResizeEnd);

		if (onResizeEnd) {
			const finalSize = {
				width: parseInt(node.style.width, 10),
				height: parseInt(node.style.height, 10)
			};
			onResizeEnd({ node, size: finalSize, mode: resizeMode });
		}

		resizeMode = null;
	}

	function handleMouseDown(event: MouseEvent): void {
		// Only process left mouse button
		if (event.button !== 0) return;

		// Ignore if clicking on a resize handle
		if (
			event.target instanceof HTMLElement &&
			event.target.classList &&
			event.target.classList.contains('resize-handle')
		) {
			return;
		}

		event.preventDefault();

		isDragging = true;
		initialPosition = { x: event.clientX, y: event.clientY };
		currentPosition = {
			x: parseInt(node.style.left, 10) || 0,
			y: parseInt(node.style.top, 10) || 0
		};

		// Apply dragging styles
		node.style.zIndex = String(zIndex);
		if (dragHandle) dragHandle.style.cursor = 'grabbing';

		// Add event listeners for dragging
		window.addEventListener('mousemove', handleMouseMove);
		window.addEventListener('mouseup', handleMouseUp);

		if (onDragStart) onDragStart({ node, position: { ...currentPosition } });
	}

	function handleMouseMove(event: MouseEvent): void {
		if (!isDragging) return;

		event.preventDefault();

		// Calculate new position
		offset = {
			x: event.clientX - initialPosition.x,
			y: event.clientY - initialPosition.y
		};

		let newLeft = currentPosition.x + offset.x;
		let newTop = currentPosition.y + offset.y;

		const rect = node.getBoundingClientRect();
		const viewportWidth = window.innerWidth;
		const viewportHeight = window.innerHeight;

		// Ensure the element stays at least partially in the viewport
		if (newLeft < 0) newLeft = 0;
		if (newTop < 0) newTop = 0;
		if (newLeft + rect.width > viewportWidth) {
			newLeft = viewportWidth - rect.width;
		}
		if (newTop + rect.height > viewportHeight) {
			newTop = viewportHeight - rect.height;
		}

		// Update position
		node.style.left = `${newLeft}px`;
		node.style.top = `${newTop}px`;
	}

	function handleMouseUp(): void {
		if (!isDragging) return;

		isDragging = false;

		// Restore styles
		if (dragHandle) dragHandle.style.cursor = 'grab';
		node.style.zIndex = initialZIndex;

		// Remove event listeners
		window.removeEventListener('mousemove', handleMouseMove);
		window.removeEventListener('mouseup', handleMouseUp);

		if (onDragEnd) {
			const finalPosition = {
				x: parseInt(node.style.left, 10),
				y: parseInt(node.style.top, 10)
			};
			onDragEnd({ node, position: finalPosition });
		}
	}

	// Setup resize touch events for mobile
	function handleResizeTouchMove(event: TouchEvent): void {
		if (!isResizing || event.touches.length !== 1) return;

		const touch = event.touches[0]!;
		const mouseEvent = new MouseEvent('mousemove', {
			clientX: touch.clientX,
			clientY: touch.clientY
		});

		handleResizeMove(mouseEvent);
	}

	function handleResizeTouchEnd(): void {
		handleResizeEnd();
	}

	// Create resize handles
	createResizeHandles();
	window.addEventListener('touchmove', handleResizeTouchMove, { passive: false });
	window.addEventListener('touchend', handleResizeTouchEnd);

	// Add initial event listener to drag handle
	if (dragHandle) {
		dragHandle.style.cursor = 'grab';
		dragHandle.addEventListener('mousedown', handleMouseDown);
	} else {
		node.addEventListener('mousedown', handleMouseDown);
	}

	// Setup touch events for mobile
	function handleTouchStart(event: TouchEvent): void {
		if (event.touches.length !== 1) return;

		const touch = event.touches[0]!;
		const mouseEvent = new MouseEvent('mousedown', {
			clientX: touch.clientX,
			clientY: touch.clientY,
			button: 0
		});

		handleMouseDown(mouseEvent);
	}

	function handleTouchMove(event: TouchEvent): void {
		if (!isDragging || event.touches.length !== 1) return;

		const touch = event.touches[0]!;
		const mouseEvent = new MouseEvent('mousemove', {
			clientX: touch.clientX,
			clientY: touch.clientY
		});

		handleMouseMove(mouseEvent);
	}

	function handleTouchEnd(): void {
		handleMouseUp();
	}

	if (dragHandle) {
		dragHandle.addEventListener('touchstart', handleTouchStart);
		window.addEventListener('touchmove', handleTouchMove, { passive: false });
		window.addEventListener('touchend', handleTouchEnd);
	} else {
		node.addEventListener('touchstart', handleTouchStart);
		window.addEventListener('touchmove', handleTouchMove, { passive: false });
		window.addEventListener('touchend', handleTouchEnd);
	}

	// Cleanup function
	return {
		update(newOptions: FloatingDivOptions): void {
			// Update options when action parameters change
			const { handleSelector: newHandleSelector, zIndex: newZIndex } = newOptions;

			// Update drag handle if selector changes
			if (newHandleSelector !== undefined && newHandleSelector !== handleSelector) {
				// Remove old listeners
				if (dragHandle) {
					dragHandle.removeEventListener('mousedown', handleMouseDown);
					dragHandle.removeEventListener('touchstart', handleTouchStart);
				} else {
					node.removeEventListener('mousedown', handleMouseDown);
					node.removeEventListener('touchstart', handleTouchStart);
				}

				// Setup new handle
				dragHandle = newHandleSelector ? node.querySelector(newHandleSelector) : node;

				// Add new listeners
				if (dragHandle) {
					dragHandle.style.cursor = 'grab';
					dragHandle.addEventListener('mousedown', handleMouseDown);
					dragHandle.addEventListener('touchstart', handleTouchStart);
				} else {
					node.addEventListener('mousedown', handleMouseDown);
					node.addEventListener('touchstart', handleTouchStart);
				}
			}

			// Update other options
			if (newZIndex !== undefined) zIndex = newZIndex;
		},

		destroy(): void {
			// Clean up all event listeners when element is removed
			if (dragHandle) {
				dragHandle.removeEventListener('mousedown', handleMouseDown);
				dragHandle.removeEventListener('touchstart', handleTouchStart);
			} else {
				node.removeEventListener('mousedown', handleMouseDown);
				node.removeEventListener('touchstart', handleTouchStart);
			}

			// Remove resize handles
			removeResizeHandles();

			// Remove all window event listeners
			window.removeEventListener('mousemove', handleMouseMove);
			window.removeEventListener('mouseup', handleMouseUp);
			window.removeEventListener('touchmove', handleTouchMove, {
				passive: false
			} as EventListenerOptions);
			window.removeEventListener('touchend', handleTouchEnd);

			window.removeEventListener('mousemove', handleResizeMove);
			window.removeEventListener('mouseup', handleResizeEnd);
			window.removeEventListener('touchmove', handleResizeTouchMove, {
				passive: false
			} as EventListenerOptions);
			window.removeEventListener('touchend', handleResizeTouchEnd);
		}
	};
}
