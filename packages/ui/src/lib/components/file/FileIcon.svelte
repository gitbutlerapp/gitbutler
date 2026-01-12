<script lang="ts">
	import { getFileIcon } from '$components/file/getFileIcon';
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		fileName: string;
		size?: number;
	}

	const { fileName, size = 14 }: Props = $props();
</script>

<i class="file-icon" style="--file-icon-size: {pxToRem(size)}rem;">
	{@html getFileIcon(fileName)}
</i>

<style lang="postcss">
	.file-icon {
		display: flex;
		width: var(--file-icon-size);
		height: var(--file-icon-size);

		--gray: #807976;
		--green: #16a34a;
		--teal: #14b8a6;
		--blue: #0ea5e9;
		--dark-blue: #0e7ce9;
		--yellow: #eab308;
		--orange: #f5700b;
		--red: #ef4444;
		--pink: #f472b6;
		--purple: #a855f7;
		--violet: #5249f8;
	}
</style>
