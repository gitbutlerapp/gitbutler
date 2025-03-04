<script lang="ts">
	import { getEditor } from '../context';

	type OnKeyDownCallback = (event: KeyboardEvent) => void;

	type Props = {
		onKeyDown?: OnKeyDownCallback;
	};

	const { onKeyDown }: Props = $props();

	const editor = getEditor();

	$effect(() => {
		return editor.registerRootListener(
			(rootElement: null | HTMLElement, prevRootElement: null | HTMLElement) => {
				if (prevRootElement !== null) {
					prevRootElement.removeEventListener('keydown', (event) => onKeyDown?.(event));
				}
				if (rootElement !== null) {
					rootElement.addEventListener('keydown', (event) => onKeyDown?.(event));
				}
			}
		);
	});
</script>
