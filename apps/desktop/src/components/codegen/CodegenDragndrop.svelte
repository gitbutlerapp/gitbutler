<script lang="ts">
	type Props = {
		attachedFiles: File[];
		processFiles: (files: FileList) => Promise<void>;
	};

	const { attachedFiles, processFiles }: Props = $props();

	let dragover = $state(false);

	// Drag handlers
	function handleDragEnter(e: DragEvent): void {
		e.preventDefault();

		dragover = true;
	}

	function handleDragLeave(e: DragEvent): void {
		e.preventDefault();

		// Only set dragover to false if we're leaving the drop area entirely
		const currentTarget = e.currentTarget as Element;
		const relatedTarget = e.relatedTarget as Node;
		if (!currentTarget?.contains(relatedTarget)) {
			dragover = false;
		}
	}

	function handleDragOver(e: DragEvent): void {
		e.preventDefault();
	}

	async function handleDrop(e: DragEvent): Promise<void> {
		e.preventDefault();
		dragover = false;

		const files = e.dataTransfer?.files;
		if (files && files.length > 0) {
			await processFiles(files);
		}
	}
</script>

<div
	class="drop-area"
	class:dragover
	class:has-files={attachedFiles.length > 0}
	ondragenter={handleDragEnter}
	ondragleave={handleDragLeave}
	ondragover={handleDragOver}
	ondrop={handleDrop}
	role="region"
	aria-label="Drag and drop files here"
>
	<div class="drop-content">
		<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M1.97153 9.87418L8.41125 3.43446C10.7126 1.13315 14.4437 1.13315 16.745 3.43446C19.0463 5.73576 19.0463 9.46691 16.745 11.7682L10.6841 17.8291C9.08015 19.4331 6.47965 19.4331 4.87571 17.8291C3.27178 16.2252 3.27178 13.6247 4.87571 12.0208L10.5578 6.33864C11.3947 5.50181 12.7514 5.50181 13.5883 6.33864C14.4251 7.17548 14.4251 8.53226 13.5883 9.3691L7.52736 15.43"
				stroke="currentColor"
				stroke-width="1.5"
				vector-effect="non-scaling-stroke"
			/>
		</svg>

		<h3 class="text-13 text-semibold">
			{#if dragover}
				Drop files here to attach
			{:else}
				Drop files to attach to your context
			{/if}
		</h3>
		<i class="text-12">Files, images or PDFs</i>
	</div>

	<!-- SVG rectangle to simulate a dashed outline with a precise dash offset. -->
	<svg width="100%" height="100%" class="animated-rectangle">
		<rect width="100%" height="100%" rx="6" ry="6" vector-effect="non-scaling-stroke" fill="none" />
	</svg>
</div>

<style lang="postcss">
	/* DROP AREA */
	.drop-area {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		min-height: 80px;
		padding: 24px;
		border-radius: var(--radius-m);
		background-color: var(--dropzone-fill);
		transition: all 0.2s ease;

		&.dragover {
			background-color: var(--dropzone-fill-hover);

			& .animated-rectangle rect {
				animation: dropzone-dash 4s linear infinite;
			}

			& .drop-content svg {
				transform: scale(1);
			}
		}

		&.has-files {
			min-height: 60px;
		}
	}

	.drop-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
		color: var(--clr-theme-pop-on-soft);
		text-align: center;
		pointer-events: none;

		& svg {
			margin-bottom: 4px;
			transform: scale(0.9);
			transition: transform var(--transition-medium);
		}

		i {
			opacity: 0.7;
		}
	}

	.animated-rectangle {
		position: absolute;
		top: 6px;
		right: 6px;
		width: calc(100% - 12px);
		height: calc(100% - 12px);
		pointer-events: none;

		& rect {
			stroke: var(--dropzone-stroke);
			stroke-width: 2px;
			stroke-dasharray: 2;
			stroke-dashoffset: 30;
			transform-origin: center;
			transition: stroke var(--transition-fast);
		}
	}
</style>
