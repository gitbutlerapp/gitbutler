<script lang="ts">
	import { getFileIcon } from '$components/file/getFileIcon';
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		fileName: string;
		size?: number;
	}

	const { fileName, size = 14 }: Props = $props();
</script>

<i class="file-icon" style="--file-icon-size: {pxToRem(size)}rem;" aria-hidden="true">
	{@html getFileIcon(fileName)}
</i>

<style lang="postcss">
	.file-icon {
		display: flex;
		width: var(--file-icon-size);
		height: var(--file-icon-size);

		--file-icon-gray: #807976;
		--file-icon-green: #16a34a;
		--file-icon-teal: #14b8a6;
		--file-icon-blue: #0ea5e9;
		--file-icon-dark-blue: #0e7ce9;
		--file-icon-yellow: #eab308;
		--file-icon-orange: #f5700b;
		--file-icon-red: #ef4444;
		--file-icon-pink: #f472b6;
		--file-icon-purple: #a855f7;
		--file-icon-violet: #5249f8;
	}
</style>
