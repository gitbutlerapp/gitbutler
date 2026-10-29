<script lang="ts" module>
	import { iconNames, type IconName } from "$lib/icons/names";
	import { pxToRem } from "$lib/utils/pxToRem";

	const modules = import.meta.glob<string>("../icons/svg/*.svg", {
		query: "?raw",
		import: "default",
		eager: true,
	});

	const icons: Record<string, string> = {};

	for (const [path, svg] of Object.entries(modules)) {
		const name = path.replace("../icons/svg/", "").replace(".svg", "");
		icons[name] = svg;
	}

	export const allIconNames = iconNames;
	// eslint-disable-next-line no-import-assign
	export type { IconName };
</script>

<script lang="ts">
	interface Props {
		name: IconName;
		size?: number;
		color?: string;
		rotate?: number;
	}

	const { name, size = 16, color, rotate }: Props = $props();

	const svg = $derived(icons[name]);
</script>

<span
	style:width="{pxToRem(size)}rem"
	style:height="{pxToRem(size)}rem"
	style:color
	class="icon"
	class:spinner={name === "spinner"}
	style:transform={rotate ? `rotate(${rotate}deg)` : undefined}
	aria-hidden="true"
>
	{@html svg}
</span>

<style>
	.icon {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		pointer-events: none;
	}

	.icon :global(svg *) {
		vector-effect: non-scaling-stroke;
	}

	.spinner {
		animation: spinning 0.75s infinite linear;
		will-change: transform;
	}

	@keyframes spinning {
		100% {
			transform: rotate(360deg);
		}
	}
</style>
