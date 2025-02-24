<script lang="ts">
	type Props = {
		element: HTMLElement;
		preserveOriginalSize?: boolean;
		originalHeight?: number;
		originalWidth?: number;
	};

	let { element, preserveOriginalSize = false, originalHeight, originalWidth }: Props = $props();

	function appendElement(node: HTMLDivElement) {
		// Append the cloned DOM element to preserve computed styles and state
		node.appendChild(element);
	}
</script>

<div
	use:appendElement
	class="reorder-clone"
	class:preserve-size={preserveOriginalSize}
	style:height={preserveOriginalSize && originalHeight ? `${originalHeight}px` : 'auto'}
	style:width={preserveOriginalSize && originalWidth ? `${originalWidth}px` : undefined}
	style:max-height={`${window.innerHeight - 100}px`}
></div>

<style>
	.reorder-clone {
		z-index: -1;
		position: absolute;
		top: -10000px;
		left: -10000px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-2);
		pointer-events: none;
	}

	.reorder-clone.preserve-size {
		/* Preserve original dimensions (e.g., for collapsed lanes) */
		flex-shrink: 0;
	}
</style>
