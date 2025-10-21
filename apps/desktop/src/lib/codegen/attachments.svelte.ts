import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { chipToasts } from '@gitbutler/ui';

export type AttachedFile = {
	id: string;
	file: File;
	preview?: string;
};

export function useFileDraggedIntoApp() {
	let dragEnterCounter = $state(0);

	// Window-level drag event handlers to show/hide drag area
	$effect(() => {
		function handleWindowDragEnter(e: DragEvent) {
			// Only show for file drags, not internal element drags
			if (e.dataTransfer?.types.includes('Files')) {
				dragEnterCounter++;
			}
		}

		function handleWindowDragLeave() {
			dragEnterCounter--;
			if (dragEnterCounter <= 0) {
				dragEnterCounter = 0;
			}
		}

		function handleWindowDrop() {
			dragEnterCounter = 0;
		}

		window.addEventListener('dragenter', handleWindowDragEnter);
		window.addEventListener('dragleave', handleWindowDragLeave);
		window.addEventListener('drop', handleWindowDrop);

		return () => {
			window.removeEventListener('dragenter', handleWindowDragEnter);
			window.removeEventListener('dragleave', handleWindowDragLeave);
			window.removeEventListener('drop', handleWindowDrop);
		};
	});

	const isDraggingFiles = $derived(dragEnterCounter > 0);

	return {
		isDraggingFiles: reactive(() => isDraggingFiles)
	};
}

const acceptedTypes = [
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
];

const maxFiles = 10;
const maxFileSizeBytes = 10 * 1024 * 1024; // 10MB

export function useAttachments() {
	let attachedFiles: AttachedFile[] = $state([]);

	function setFiles(files: AttachedFile[]) {
		attachedFiles = files;
	}

	function removeFile(fileId: string) {
		attachedFiles = attachedFiles.filter((f) => f.id !== fileId);
	}

	async function processFiles(files: FileList): Promise<void> {
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
		setFiles([...attachedFiles, ...newFiles]);
	}

	return {
		attachedFiles: reactive(() => attachedFiles),
		setFiles,
		removeFile,
		processFiles
	};
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
