<script lang="ts" module>
	export type DropFileResult = {
		name: string;
		url: string;
		isImage: boolean;
	};
</script>

<script lang="ts">
	import { getEditor } from '../context';
	import { insertTextAtCaret } from '../selection';
	import { COMMAND_PRIORITY_CRITICAL, DROP_COMMAND } from 'lexical';

	type Props = {
		onDrop: (files: FileList | undefined) => Promise<DropFileResult[]>;
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
		results.forEach((result) => {
			const embed = embedDroppedFile(result);
			insertTextAtCaret(editor, `${embed}\n`);
		});
	}

	$effect(() => {
		return editor.registerCommand(
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
	});

	export async function handleFileUpload(files: FileList | null) {
		if (!files) return;
		await handleDrop(files);
	}
</script>
