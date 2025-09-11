<script lang="ts">
	import { getFileIcon } from '$components/file/getFileIcon';
	import { fileIcons } from '$components/file/fileIcons';
	import { convertToBase64 } from '$lib/utils/convertToBase64';
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		fileName: string;
		size?: number;
		fileType?: 'regular' | 'executable' | 'symlink' | 'submodule';
		/**
		 * @deprecated Use fileType prop instead
		 */
		executable?: boolean;
	}

	const { fileName, size = 16, fileType = 'regular', executable = false }: Props = $props();

	// Determine the actual file type, with backward compatibility
	const actualFileType = $derived(() => {
		if (executable && fileType === 'regular') {
			return 'executable';
		}
		return fileType;
	});

	// Get the appropriate icon based on file type
	const iconSrc = $derived(() => {
		const type = actualFileType();
		
		// For symlinks and submodules, always use their specific icons
		if (type === 'symlink') {
			const icon = fileIcons['symlink'];
			return `data:image/svg+xml;base64,${convertToBase64(icon)}`;
		}
		
		if (type === 'submodule') {
			const icon = fileIcons['submodule'];
			return `data:image/svg+xml;base64,${convertToBase64(icon)}`;
		}
		
		// For regular and executable files, use the filename-based icon
		return getFileIcon(fileName);
	});

	// Determine if we should show the executable overlay
	const showExecutableOverlay = $derived(() => {
		const type = actualFileType();
		return type === 'executable';
	});

	// Get the executable overlay icon
	const executableOverlaySrc = $derived(() => {
		if (!showExecutableOverlay()) return '';
		const icon = fileIcons['executable-overlay'];
		return `data:image/svg+xml;base64,${convertToBase64(icon)}`;
	});
</script>

<div class="file-icon-container" style:--file-icon-size="{pxToRem(size)}rem">
	<img
		draggable="false"
		src={iconSrc()}
		alt=""
		class="file-icon"
	/>
	
	{#if showExecutableOverlay()}
		<img
			draggable="false"
			src={executableOverlaySrc()}
			alt="executable"
			class="executable-overlay"
		/>
	{/if}
</div>

<style lang="postcss">
	.file-icon-container {
		position: relative;
		display: inline-block;
		width: var(--file-icon-size);
		height: var(--file-icon-size);
	}

	.file-icon {
		width: 100%;
		height: 100%;
	}

	.executable-overlay {
		position: absolute;
		top: -2px;
		right: -2px;
		width: 50%;
		height: 50%;
		z-index: 1;
	}
</style>
