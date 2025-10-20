<script lang="ts">
	import { chipToasts, FileIcon, Icon } from '@gitbutler/ui';
	import { fly } from 'svelte/transition';

	interface AttachedFile {
		id: string;
		file: File;
		preview?: string;
	}

	interface Props {
		attachedFiles?: AttachedFile[];
		onFilesChanged?: (files: AttachedFile[]) => void;
		maxFiles?: number;
		maxFileSizeBytes?: number;
		acceptedTypes?: string[];
		showDropArea?: boolean;
	}

	let {
		attachedFiles = $bindable([]),
		onFilesChanged,
		maxFiles = 10,
		maxFileSizeBytes = 10 * 1024 * 1024, // 10MB
		acceptedTypes = [
			'image/*',
			'text/*',
			'.pdf',
			'.doc',
			'.docx',
			'.md',
			'.tsx',
			'.ts',
			'.jsx',
			'.js',
			'.vue',
			'.svelte'
		],
		showDropArea = false
	}: Props = $props();

	let dragover = $state(false);

	// Generate preview for image files
	async function generatePreview(file: File): Promise<string | undefined> {
		if (file.type.startsWith('image/')) {
			return new Promise((resolve) => {
				const reader = new FileReader();
				reader.onload = (e) => resolve(e.target?.result as string);
				reader.onerror = () => resolve(undefined);
				reader.readAsDataURL(file);
			});
		}
		return undefined;
	}

	// Validate file
	function validateFile(file: File): string | null {
		if (file.size > maxFileSizeBytes) {
			return `File "${file.name}" is too large. Maximum size is ${Math.round(maxFileSizeBytes / 1024 / 1024)}MB.`;
		}

		if (acceptedTypes.length > 0) {
			const isAccepted = acceptedTypes.some((type) => {
				if (type.startsWith('.')) {
					return file.name.toLowerCase().endsWith(type.toLowerCase());
				}
				if (type.includes('*')) {
					const baseType = type.split('/')[0];
					return baseType ? file.type.startsWith(baseType) : false;
				}
				return file.type === type;
			});

			if (!isAccepted) {
				return `File "${file.name}" is not an accepted file type.`;
			}
		}

		return null;
	}

	// Process files
	async function processFiles(files: FileList | File[]): Promise<void> {
		const fileArray = Array.from(files);

		// Check total file count
		if (attachedFiles.length + fileArray.length > maxFiles) {
			chipToasts.error(
				`Cannot add ${fileArray.length} files. Maximum of ${maxFiles} files allowed.`
			);
			return;
		}

		// Validate and process each file
		const newFiles: AttachedFile[] = [];
		for (const file of fileArray) {
			const error = validateFile(file);
			if (error) {
				chipToasts.error(error);
				return;
			}

			// Check for duplicates
			const isDuplicate = attachedFiles.some(
				(existing) =>
					existing.file.name === file.name &&
					existing.file.size === file.size &&
					existing.file.lastModified === file.lastModified
			);

			if (isDuplicate) {
				chipToasts.error(`File "${file.name}" is already attached.`);
				return;
			}

			const preview = await generatePreview(file);
			newFiles.push({
				id: `${file.name}-${Date.now()}-${Math.random()}`,
				file,
				preview
			});
		}

		// Add new files
		attachedFiles = [...attachedFiles, ...newFiles];
		onFilesChanged?.(attachedFiles);
	}

	// Remove file
	function removeFile(fileId: string): void {
		attachedFiles = attachedFiles.filter((f) => f.id !== fileId);
		onFilesChanged?.(attachedFiles);
	}

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

<div class="dragndrop-container">
	<!-- Drop Area -->
	{#if showDropArea}
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
				<svg
					width="20"
					height="20"
					viewBox="0 0 20 20"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
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
				<rect
					width="100%"
					height="100%"
					rx="6"
					ry="6"
					vector-effect="non-scaling-stroke"
					fill="none"
				/>
			</svg>
		</div>
	{/if}

	<!-- Attached Files -->
	{#if attachedFiles.length > 0}
		<div class="attached-files">
			{#each attachedFiles as attachedFile (attachedFile.id)}
				<div class="file-item" in:fly={{ y: 10, duration: 150 }}>
					<div class="file-content">
						{#if attachedFile.preview}
							<img src={attachedFile.preview} alt={attachedFile.file.name} class="file-preview" />
						{:else}
							<FileIcon fileName={attachedFile.file.name} />
						{/if}

						<span class="text-12 text-semibold file-name" title={attachedFile.file.name}>
							{attachedFile.file.name}
						</span>
					</div>

					<button
						type="button"
						class="remove-button"
						onclick={() => removeFile(attachedFile.id)}
						aria-label="Remove {attachedFile.file.name}"
						title="Remove file"
					>
						<Icon name="cross-small" />
					</button>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style lang="postcss">
	.dragndrop-container {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
	}

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

	/* ATTACHED FILES */
	.attached-files {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.file-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: var(--size-button);
		padding-right: 2px;
		padding-left: 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--size-button);
	}

	.file-content {
		display: flex;
		flex: 1;
		align-items: center;
		gap: 6px;
	}

	.file-preview {
		width: 20px;
		height: 20px;
		margin-left: -2px;
		object-fit: cover;
		border-radius: 20px;
		background-color: var(--clr-bg-3);
	}

	.file-name {
		max-width: 400px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.remove-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-button);
		height: var(--size-button);
		color: var(--clr-text-3);
		transition: all 0.2s ease;

		&:hover {
			background-color: var(--clr-bg-error);
			color: var(--clr-text-error);
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
