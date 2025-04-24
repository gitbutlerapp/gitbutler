<script lang="ts" module>
	export type DropFileResult = {
		name: string;
		url: string;
		isImage: boolean;
	};
</script>

<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { insertTextAtCaret } from '$lib/richText/selection';
	import { COMMAND_PRIORITY_CRITICAL, DROP_COMMAND, PASTE_COMMAND } from 'lexical';

	type Props = {
		onDrop: (files: FileList | undefined) => Promise<DropFileResult[] | undefined>;
	};

	const { onDrop }: Props = $props();

	const editor = getEditor();

	function embedLinkMD(url: string, text: string) {
		return `[${text}](${url})`;
	}

	function embedImageMD(url: string, alt: string) {
		return `![${alt}](${url})`;
	}

	function embedDroppedFile(file: DropFileResult) {
		return file.isImage ? embedImageMD(file.url, file.name) : embedLinkMD(file.url, file.name);
	}

	async function handleDrop(files: FileList | undefined) {
		if (!files) return;

		const results = await onDrop(files);
		if (!results) return;
		results.forEach((result) => {
			const embed = embedDroppedFile(result);
			insertTextAtCaret(editor, `${embed}\n`);
		});
	}

	$effect(() => {
		const unregisterDrop = editor.registerCommand(
			DROP_COMMAND,
			(e) => {
				e.preventDefault();
				e.stopPropagation();

				const files = e.dataTransfer?.files;
				handleDrop(files);
				return true;
			},
			COMMAND_PRIORITY_CRITICAL
		);

		const unregisterPaste = editor.registerCommand(
			PASTE_COMMAND,
			(e) => {
				const clipboardEvent = e as ClipboardEvent;
				const files = clipboardEvent.clipboardData?.files;

				if (files && files.length > 0) {
					e.preventDefault();
					e.stopPropagation();

					handleDrop(files);

					return true;
				}

				return false;
			},
			COMMAND_PRIORITY_CRITICAL
		);

		return () => {
			unregisterDrop();
			unregisterPaste();
		};
	});

	export async function handleFileUpload(files: FileList | null) {
		if (!files) return;
		await handleDrop(files);
	}
</script>
