import { type CommitStatusType } from '$lib/commits/commit';
import { FileDropData, ChangeDropData, type DropData } from '$lib/dragging/draggables';
import { dropzoneRegistry } from '$lib/dragging/dropzone';
import { getFileIcon } from '@gitbutler/ui/file/getFileIcon';
import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

// Added to element being dragged (not the clone that follows the cursor).
const DRAGGING_CLASS = 'dragging';

export type NonDraggableConfig = {
	disabled: true;
};

export type DraggableConfig = {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly label?: string;
	readonly filePath?: string;
	readonly sha?: string;
	readonly date?: string;
	readonly authorImgUrl?: string;
	readonly commitType?: CommitStatusType;
	readonly data?: DropData;
	readonly viewportId?: string;
	readonly chipType?: 'file' | 'hunk';
};

function createElement<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	classNames: string[],
	textContent?: string,
	src?: string
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	el.classList.add(...classNames);
	if (textContent) el.textContent = textContent;
	if (src) (el as HTMLImageElement).src = src;
	return el;
}

function setupDragHandlers(
	node: HTMLElement,
	opts: DraggableConfig | NonDraggableConfig,
	createClone: (opts: DraggableConfig, selectedElements: Element[]) => HTMLElement | undefined,
	params: {
		handlerWidth: boolean;
		maxHeight?: number;
	} = {
		handlerWidth: false
	}
):
	| {
			update: (opts: DraggableConfig) => void;
			destroy: () => void;
	  }
	| undefined {
	if (opts.disabled) return;
	let dragHandle: HTMLElement | null;
	let clone: HTMLElement | undefined;
	let selectedElements: Element[] = [];

	function handleMouseDown(e: MouseEvent) {
		dragHandle = e.target as HTMLElement;
	}

	function handleDragStart(e: DragEvent) {
		if (opts.disabled) return;
		e.stopPropagation();

		if (dragHandle && dragHandle.dataset.noDrag !== undefined) {
			e.preventDefault();
			return false;
		}

		const parentNode = node.parentElement?.parentElement;
		if (!parentNode) {
			console.error('draggable parent node not found');
			return;
		}

		// This should be deleted once V3 design has shipped.
		if (opts.data instanceof FileDropData) {
			selectedElements = [];
			for (const file of opts.data.files) {
				const element = parentNode.querySelector(`[data-file-id="${file.id}"]`);
				if (element) {
					if (file.locked) {
						element.classList.add('locked-file-animation');
						element.addEventListener(
							'animationend',
							() => {
								element.classList.remove('locked-file-animation');
							},
							{ once: true }
						);
					}
					selectedElements.push(element);
				}
			}
		}

		if (opts.data instanceof ChangeDropData) {
			selectedElements = [];
			for (const path of opts.data.changedPaths(opts.data.selectionId)) {
				// Path is sufficient as a key since we query the parent container.
				const element = parentNode.querySelector(`[data-file-id="${path}"]`);
				if (element) {
					selectedElements.push(element);
				}
			}
		}

		if (selectedElements.length === 0) {
			selectedElements = [node];
		}

		for (const element of selectedElements) {
			element.classList.add(DRAGGING_CLASS);
		}

		for (const dropzone of Array.from(dropzoneRegistry.values())) {
			dropzone.activate(opts.data);
		}

		clone = createClone(opts, selectedElements);
		if (clone) {
			if (params.handlerWidth) {
				clone.style.width = node.clientWidth + 'px';
			}
			if (params.maxHeight) {
				clone.style.maxHeight = pxToRem(params.maxHeight) as string;
			}
			document.body.appendChild(clone);

			if (e.dataTransfer) {
				if (params.handlerWidth) {
					e.dataTransfer.setDragImage(clone, e.offsetX, e.offsetY);
				} else {
					e.dataTransfer.setDragImage(clone, clone.offsetWidth - 20, 25);
				}

				// Get chromium to fire dragover & drop events
				// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
				e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
				e.dataTransfer.effectAllowed = 'uninitialized';
			}
		}
	}

	const viewport = opts.viewportId ? document.getElementById(opts.viewportId) : null;
	const triggerRange = 150;
	const timerShutter = 700;
	let timeoutId: undefined | ReturnType<typeof setTimeout> = undefined;

	function loopScroll(viewport: HTMLElement, direction: 'left' | 'right', scrollSpeed: number) {
		viewport.scrollBy({
			left: direction === 'left' ? -scrollSpeed : scrollSpeed,
			behavior: 'smooth'
		});

		timeoutId = setTimeout(() => loopScroll(viewport, direction, scrollSpeed), timerShutter);
	}

	function handleDrag(e: DragEvent) {
		e.preventDefault();

		if (!viewport) return;

		const scrollSpeed = (viewport.clientWidth || 500) / 3; // Fine-tune the scroll speed
		const viewportWidth = viewport.clientWidth;
		const relativeX = e.clientX - viewport.getBoundingClientRect().left;

		if (relativeX < triggerRange && viewport.scrollLeft > 0) {
			// Start scrolling to the left
			if (!timeoutId) {
				loopScroll(viewport, 'left', scrollSpeed);
			}
		} else if (relativeX > viewportWidth - triggerRange) {
			// Start scrolling to the right
			if (!timeoutId) {
				loopScroll(viewport, 'right', scrollSpeed);
			}
		} else {
			// Stop scrolling if not in the scrollable range
			if (timeoutId) {
				clearTimeout(timeoutId);
				timeoutId = undefined;
			}
		}
	}

	function handleDragEnd(e: DragEvent) {
		e.stopPropagation();
		if (clone) clone.remove();
		selectedElements.forEach((el) => el.classList.remove(DRAGGING_CLASS));
		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.deactivate();
		});

		if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = undefined;
		}
	}

	function setup(newOpts: DraggableConfig) {
		if (newOpts.disabled) return;
		opts = newOpts;
		node.draggable = true;

		node.addEventListener('dragstart', handleDragStart);
		node.addEventListener('drag', handleDrag);
		node.addEventListener('dragend', handleDragEnd);
		node.addEventListener('mousedown', handleMouseDown, { capture: false });
	}

	function clean() {
		node.draggable = false;
		node.removeEventListener('dragstart', handleDragStart);
		node.removeEventListener('drag', handleDrag);
		node.removeEventListener('dragend', handleDragEnd);
		node.removeEventListener('mousedown', handleMouseDown, { capture: false });
	}

	setup(opts);

	return {
		update(newOpts: DraggableConfig) {
			clean();
			setup(newOpts);
		},
		destroy() {
			clean();
		}
	};
}

//////////////////////////
//// COMMIT DRAGGABLE ////
//////////////////////////

export function createCommitElement(
	commitType: CommitStatusType | undefined,
	label: string | undefined,
	sha: string | undefined,
	date: string | undefined,
	authorImgUrl: string | undefined
): HTMLDivElement {
	const cardEl = createElement('div', ['draggable-commit']);
	const labelEl = createElement('span', ['text-13', 'text-bold'], label || 'Empty commit');
	const infoEl = createElement('div', ['draggable-commit-info', 'text-11']);
	const authorImgEl = createElement(
		'img',
		['draggable-commit-author-img'],
		undefined,
		authorImgUrl
	);
	const shaEl = createElement('span', ['draggable-commit-info-text'], sha);
	const dateAndAuthorEl = createElement('span', ['draggable-commit-info-text'], date);

	if (commitType) {
		const indicatorClass = `draggable-commit-${commitType}`;
		labelEl.classList.add('draggable-commit-indicator', indicatorClass);
	}

	cardEl.appendChild(labelEl);
	infoEl.appendChild(authorImgEl);
	infoEl.appendChild(shaEl);
	infoEl.appendChild(dateAndAuthorEl);
	cardEl.appendChild(infoEl);
	return cardEl;
}

export function draggableCommit(
	node: HTMLElement,
	initialOpts: DraggableConfig | NonDraggableConfig
) {
	function createClone(opts: DraggableConfig) {
		if (opts.disabled) return;
		return createCommitElement(opts.commitType, opts.label, opts.sha, opts.date, opts.authorImgUrl);
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: true
	});
}

//////////////////
/// DRAG CHIPS ///
//////////////////

function createFileChipContainer(
	label: string | undefined,
	filePath: string | undefined
): HTMLDivElement {
	const containerEl = createElement('div', ['dragchip-file-container']);

	if (filePath) {
		const fileIcon = getFileIcon(filePath);
		const iconEl = createElement('img', ['dragchip-file-icon'], undefined, fileIcon);
		containerEl.appendChild(iconEl);
	}

	const fileNameEl = createElement(
		'span',
		['text-12', 'text-semibold', 'dragchip-file-name'],
		label || 'Empty file'
	);
	containerEl.appendChild(fileNameEl);

	return containerEl;
}

function createHunkChipContainer(label: string | undefined): HTMLDivElement {
	const containerEl = createElement('div', ['dragchip-hunk-container']);

	const hunkDecoIndatorEl = createElement('div', ['dragchip-hunk-decorator']);
	hunkDecoIndatorEl.textContent = '〈/〉';
	containerEl.appendChild(hunkDecoIndatorEl);

	const hunkLabelEl = createElement('span', ['dragchip-hunk-label'], label || 'Empty hunk');
	containerEl.appendChild(hunkLabelEl);

	return containerEl;
}

export function createChipsElement(
	opt: {
		childrenAmount: number;
		label: string | undefined;
		filePath: string | undefined;
		chipType: 'file' | 'hunk';
	} = {
		childrenAmount: 1,
		label: undefined,
		filePath: undefined,
		chipType: 'file'
	}
): HTMLDivElement {
	const containerEl = createElement('div', ['dragchip-container']);
	const chipEl = createElement('div', ['dragchip']);
	containerEl.appendChild(chipEl);

	if (opt.chipType === 'file') {
		const fileChipContainer = createFileChipContainer(opt.label, opt.filePath);
		chipEl.appendChild(fileChipContainer);
	} else if (opt.chipType === 'hunk') {
		const hunkChipContainer = createHunkChipContainer(opt.label);

		chipEl.appendChild(hunkChipContainer);
	}

	if (opt.childrenAmount > 1) {
		const amountTag = createElement(
			'div',
			['text-11', 'text-bold', 'dragchip-amount'],
			opt.childrenAmount.toString()
		);
		chipEl.appendChild(amountTag);
	}

	if (opt.childrenAmount === 2) {
		containerEl.classList.add('dragchip-two');
	} else if (opt.childrenAmount > 2) {
		containerEl.classList.add('dragchip-multiple');
	}

	return containerEl;
}

export function draggableChips(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig, selectedElements: Element[]) {
		if (opts.disabled) return;
		return createChipsElement({
			childrenAmount: selectedElements.length,
			label: opts.label,
			filePath: opts.filePath,
			chipType: opts.chipType || 'file'
		});
	}
	return setupDragHandlers(node, initialOpts, createClone);
}

////////////////////////////
//// GENERAL DRAG CLONE ////
////////////////////////////

export function cloneElement(node: HTMLElement) {
	const cloneEl = node.cloneNode(true) as HTMLElement;

	// exclude all ignored elements from the clone
	const ignoredElements = Array.from(cloneEl.querySelectorAll('[data-remove-from-draggable]'));
	ignoredElements.forEach((el) => el.remove());

	return cloneEl;
}
