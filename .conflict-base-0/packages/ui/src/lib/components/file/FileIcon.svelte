<script lang="ts">
	import { getFileIcon } from '$components/file/getFileIcon';
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		fileName: string;
		size?: number;
	}

	const { fileName, size = 16 }: Props = $props();
</script>

<img
	draggable="false"
	src={getFileIcon(fileName)}
	alt=""
	class="file-icon"
	style:--file-icon-size="{pxToRem(size)}rem"
/>

<style lang="postcss">
	.file-icon {
		width: var(--file-icon-size);
		height: var(--file-icon-size);
	}
</style>
