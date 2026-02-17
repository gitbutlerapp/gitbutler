<script lang="ts">
	import { getFileIcon } from "$components/file/getFileIcon";

	interface Props {
		fileName: string;
		color?: string;
	}

	const { fileName, color }: Props = $props();
</script>

<i class="file-icon" style:--file-icon-custom-color={color} aria-hidden="true">
	{@html getFileIcon(fileName)}
</i>

<style lang="postcss">
	.file-icon {
		display: flex;
		width: 14px;
		height: 14px;

		--file-icon-gray: var(--file-icon-custom-color, #807976);
		--file-icon-green: var(--file-icon-custom-color, #16a34a);
		--file-icon-teal: var(--file-icon-custom-color, #14b8a6);
		--file-icon-blue: var(--file-icon-custom-color, #0ea5e9);
		--file-icon-dark-blue: var(--file-icon-custom-color, #0e7ce9);
		--file-icon-yellow: var(--file-icon-custom-color, #eab308);
		--file-icon-orange: var(--file-icon-custom-color, #f5700b);
		--file-icon-red: var(--file-icon-custom-color, #ef4444);
		--file-icon-pink: var(--file-icon-custom-color, #f472b6);
		--file-icon-purple: var(--file-icon-custom-color, #a855f7);
		--file-icon-violet: var(--file-icon-custom-color, #5249f8);
	}
</style>
