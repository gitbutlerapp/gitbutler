<script lang="ts" module>
	export interface Props {
		margin?: number;
		noLine?: boolean;
		dotted?: boolean;
	}
</script>

<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

	const { margin = 12, noLine = false, dotted = false }: Props = $props();

	function getMargins() {
		if (margin === undefined) {
			return '';
		}

		return `margin-top: ${pxToRem(margin)}; margin-bottom: ${pxToRem(margin)};`;
	}
</script>

<div
	class="divider"
	style={getMargins()}
	class:line={!noLine && !dotted}
	class:dotted={!noLine && dotted}
></div>

<style lang="postcss">
	.divider {
		height: 1px;
		width: 100%;
		opacity: 0.13;
	}

	.divider.line {
		background-color: var(--clr-scale-ntrl-0);
	}

	.divider.dotted {
		opacity: 0.2;
		background: repeating-linear-gradient(
			90deg,
			transparent,
			transparent 2px,
			var(--clr-scale-ntrl-0) 2px,
			var(--clr-scale-ntrl-0) 4px
		);
	}
</style>
