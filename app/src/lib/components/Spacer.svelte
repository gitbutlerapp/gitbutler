<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';
	export let margin: number | undefined = 16;
	export let noLine = false;

	function getMargins() {
		if (margin === undefined) {
			return '';
		}

		return `margin-top: ${pxToRem(margin)}; margin-bottom: ${pxToRem(margin)};`;
	}
</script>

<div class="divider" style={getMargins()} class:line={!noLine}></div>

<style lang="postcss">
	.divider {
		height: 1px;
		width: 100%;
		opacity: 0.15;
	}

	.divider.line {
		border-bottom: 1px solid var(--clr-scale-ntrl-0);
	}
</style>
