<script lang="ts">
	import CodegenDragndrop from '$components/codegen/CodegenDragndrop.svelte';
	import CodegenInputQueued from '$components/codegen/CodegenInputQueued.svelte';
	import { Tooltip, Textarea, AsyncButton } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	interface AttachedFile {
		id: string;
		file: File;
		preview?: string;
	}

	interface Props {
		value: string;
		loading: boolean;
		compacting: boolean;
		onsubmit: () => Promise<void>;
		onAbort?: () => Promise<void>;
		actionsOnLeft: Snippet<[{ triggerFileSelection: () => void }]>;
		actionsOnRight: Snippet;
		projectId: string;
		selectedBranch: { stackId: string; head: string } | undefined;
		onChange: (value: string) => void;
		sessionKey?: string; // Used to trigger refocus when switching sessions
		attachedFiles?: AttachedFile[];
		onFilesChanged?: (files: AttachedFile[]) => void;
		showFileAttachment?: boolean;
	}
	let {
		value = $bindable(),
		loading,
		compacting,
		onsubmit,
		onAbort,
		actionsOnLeft,
		actionsOnRight,
		projectId,
		selectedBranch,
		onChange,
		sessionKey,
		attachedFiles = $bindable([]),
		onFilesChanged,
		showFileAttachment = true
	}: Props = $props();

	let textareaRef = $state<HTMLTextAreaElement>();
	let fileInputRef = $state<HTMLInputElement>();
	let isDraggingOverWindow = $state(false);
	let dragEnterCounter = $state(0);

	// Function to trigger file selection dialog
	function triggerFileSelection() {
		if (!loading && fileInputRef) {
			fileInputRef.click();
		}
	}

	// Handle file input change
	async function handleFileInputChange() {
		if (fileInputRef?.files && fileInputRef.files.length > 0) {
			const newFiles: AttachedFile[] = [];

			for (const file of Array.from(fileInputRef.files)) {
				// Generate preview for image files
				let preview: string | undefined = undefined;
				if (file.type.startsWith('image/')) {
					preview = await new Promise<string>((resolve) => {
						const reader = new FileReader();
						reader.onload = (e) => resolve(e.target?.result as string);
						reader.onerror = () => resolve('');
						reader.readAsDataURL(file);
					});
				}

				// Check for duplicates
				const isDuplicate = attachedFiles.some(
					(existing) =>
						existing.file.name === file.name &&
						existing.file.size === file.size &&
						existing.file.lastModified === file.lastModified
				);

				if (!isDuplicate) {
					newFiles.push({
						id: `${file.name}-${Date.now()}-${Math.random()}`,
						file,
						preview
					});
				}
			}

			if (newFiles.length > 0) {
				attachedFiles = [...attachedFiles, ...newFiles];
				onFilesChanged?.(attachedFiles);
			}

			// Reset input
			fileInputRef.value = '';
		}
	}

	// Window-level drag event handlers to show/hide drag area
	$effect(() => {
		function handleWindowDragEnter(e: DragEvent) {
			// Only show for file drags, not internal element drags
			if (e.dataTransfer?.types.includes('Files')) {
				dragEnterCounter++;
				isDraggingOverWindow = true;
			}
		}

		function handleWindowDragLeave() {
			dragEnterCounter--;
			if (dragEnterCounter <= 0) {
				dragEnterCounter = 0;
				isDraggingOverWindow = false;
			}
		}

		function handleWindowDrop() {
			dragEnterCounter = 0;
			isDraggingOverWindow = false;
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

	// Focus when component mounts or when session changes
	$effect(() => {
		if (textareaRef && sessionKey) {
			// Additional focus with longer delay to handle parent component timing
			setTimeout(() => {
				textareaRef?.focus();
			}, 0);
		}
	});

	$effect(() => {
		onChange(value);
	});

	let showAbortButton = $state(false);

	$effect(() => {
		// Show abort button if loading for more than 1 second
		if (loading && onAbort) {
			const timer = setTimeout(() => {
				showAbortButton = true;
			}, 1000);

			return () => {
				clearTimeout(timer);
				showAbortButton = false;
			};
		} else {
			showAbortButton = false;
		}
	});

	async function handleSubmit() {
		await onsubmit();
	}

	async function handleKeypress(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();

			if (value.trim().length === 0) return;

			await handleSubmit();
		}
	}

	function handleDialogClick(e: MouseEvent) {
		// Don't focus if clicking on buttons or other interactive elements
		if (e.target !== e.currentTarget) return;
		textareaRef?.focus();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-wrapper">
	<!-- Hidden file input for attachment button -->
	<input
		bind:this={fileInputRef}
		type="file"
		multiple
		accept="image/*,text/*,.pdf,.doc,.docx,.md"
		style="display: none"
		onchange={handleFileInputChange}
	/>

	<div class="text-input dialog-input" onkeypress={handleKeypress} onclick={handleDialogClick}>
		<CodegenInputQueued {projectId} {selectedBranch} />

		{#if showFileAttachment && !compacting && (isDraggingOverWindow || attachedFiles.length > 0)}
			<div class="file-attachment-section">
				<CodegenDragndrop bind:attachedFiles {onFilesChanged} showDropArea={isDraggingOverWindow} />
			</div>
		{/if}

		<Textarea
			bind:textBoxEl={textareaRef}
			bind:value
			placeholder="What would you like to make..."
			borderless
			maxRows={10}
			minRows={2}
			onkeydown={(e) => {
				// Global gotakey on the button doesn't work inside textarea, so we handle it here
				if (e.key === 'c' && e.ctrlKey && onAbort) {
					e.preventDefault();
					onAbort();
				}
			}}
		/>

		<div class="actions">
			<div class="actions-group">
				{@render actionsOnLeft({ triggerFileSelection })}
			</div>

			<div class="actions-group">
				{@render actionsOnRight()}

				<div class="actions-separator"></div>

				{#if !compacting && showAbortButton && onAbort}
					<div class="flex" in:fade={{ duration: 150 }} out:fade={{ duration: 100 }}>
						<AsyncButton
							kind="outline"
							style="error"
							action={onAbort}
							icon="stop"
							hotkey="⌃C"
							reversedDirection
						>
							Stop
						</AsyncButton>
					</div>
				{/if}

				<Tooltip
					text={loading ? 'Processing...' : value.trim().length === 0 ? 'Type a message' : 'Send ↵'}
				>
					<button
						class="send-button"
						type="button"
						disabled={value.trim().length === 0}
						class:loading
						style="pop"
						onclick={handleSubmit}
						aria-label="Send"
					>
						<svg
							class="circle-icon"
							viewBox="0 0 18 18"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<circle
								vector-effect="non-scaling-stroke"
								cx="9"
								cy="9"
								r="8.25"
								stroke="currentColor"
							/>
						</svg>

						<svg
							class="arrow-icon"
							viewBox="0 0 16 16"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								vector-effect="non-scaling-stroke"
								d="M12.0195 8L8.72664 4.70711C8.33611 4.31658 7.70295 4.31658 7.31242 4.70711L4.01953 8"
								stroke="currentColor"
								stroke-width="1.5"
							/>
							<path
								d="M8.01953 4L8.01953 12"
								stroke="currentColor"
								stroke-width="1.5"
								vector-effect="non-scaling-stroke"
							/>
						</svg>
					</button>
				</Tooltip>
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.dialog-wrapper {
		flex-shrink: 0;
		width: 100%;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}

	.dialog-input {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		transition: border-color var(--transition-fast);
	}

	.file-attachment-section {
		padding: 12px;
		padding-bottom: 4px;
	}

	.actions {
		display: flex;
		z-index: 2;
		position: relative;
		bottom: 0;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 12px;
		padding-top: 10px;
		gap: 8px;
		pointer-events: none;

		&::after {
			position: absolute;
			top: 0;
			right: 12px;
			left: 12px;
			height: 1px;
			background-color: var(--clr-border-3);
			content: '';
		}
	}

	.actions-group {
		display: flex;
		position: relative;
		gap: 4px;
		pointer-events: all;
	}

	.actions-separator {
		width: 1px;
		margin: 0 5px;
		background-color: var(--clr-border-3);
	}

	/* SEND BUTTON */
	.send-button {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		height: var(--size-button);
		padding: 0 6px;
		border-radius: var(--radius-btn);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		transition:
			background-color 0.2s ease-in-out,
			transform 0.2s ease-in-out;

		&:not(:disabled):hover {
			transform: translateY(-2px);
			background-color: var(--clr-theme-pop-element-hover);

			.arrow-icon {
				transform: translate(-50%, -50%) translateY(1px);
			}
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}
	}

	.circle-icon {
		width: 18px;
		height: 18px;
		stroke-width: 1.5;
		transition: transform 0.2s;
	}

	.arrow-icon {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 16px;
		height: 16px;
		transform: translate(-50%, -50%);
		transition: transform 0.2s;
	}
</style>
