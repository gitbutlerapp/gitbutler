<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		margin?: number;
		noLine?: boolean;
		dotted?: boolean;
	}

	const { margin = 12, noLine = false, dotted = false }: Props = $props();

	function getMargins() {
		if (margin === undefined) {
			return '';
		}

		return `margin-top: ${pxToRem(margin)}rem; margin-bottom: ${pxToRem(margin)}rem;`;
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
		width: 100%;
		height: 1px;
		opacity: 0.13;
	}

	.divider.line {
		background-color: var(--clr-text-1);
	}

	.divider.dotted {
		background-image: linear-gradient(to right, var(--clr-text-1) 50%, transparent 50%);
		background-size: 4px 1px;
		opacity: 0.2;
	}
</style>
