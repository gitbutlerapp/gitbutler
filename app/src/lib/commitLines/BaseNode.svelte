<script lang="ts">
	import type { BaseNode, Color } from '$lib/commitLines/types';
	interface Props {
		baseNode: BaseNode;
		color: Color;
	}

	const { baseNode: _baseNode, color: _color }: Props = $props();
</script>

<div class="container">
	<div class="node">
		<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none">
			<path
				fill-rule="evenodd"
				clip-rule="evenodd"
				d="M4.32501 7.25C4.67247 5.53832 6.18578 4.25 8 4.25C9.81422 4.25 11.3275 5.53832 11.675 7.25H14V8.75H11.675C11.3275 10.4617 9.81422 11.75 8 11.75C6.18578 11.75 4.67247 10.4617 4.32501 8.75H2V7.25H4.32501ZM8 5.75C6.75736 5.75 5.75 6.75736 5.75 8C5.75 9.24264 6.75736 10.25 8 10.25C9.24264 10.25 10.25 9.24264 10.25 8C10.25 6.75736 9.24264 5.75 8 5.75Z"
				fill="white"
			/>
		</svg>
	</div>
</div>

<style lang="postcss">
	.container {
		z-index: var(--z-lifted);

		& .node {
			height: 16px;
			width: 16px;

			margin-top: var(--base-icon-top);
			margin-bottom: -8px;
			margin-left: -9px;
			margin-right: -7px;

			background-color: var(--clr-commit-remote);
			border-radius: 6px;

			display: flex;
			align-items: center;
			justify-content: center;

			transition: margin-top var(--transition-medium);
		}
	}
</style>
