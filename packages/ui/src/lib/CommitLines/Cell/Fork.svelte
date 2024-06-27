<script lang="ts">
	import type { Color, Style } from '$lib/commitLines/types';

	interface Props {
		color: Color;
		style?: Style;
		isBottom?: boolean;
	}

	const { color, style, isBottom = false }: Props = $props();
</script>

<div
	class="commit-line fork"
	class:none={color === 'none'}
	class:remote={color === 'remote'}
	class:local={color === 'local'}
	class:local-and-remote={color === 'localAndRemote'}
	class:shadow={color === 'shadow'}
	class:integrated={color === 'integrated'}
	class:dashed={style === 'dashed'}
>
	<div class="row top-row">
		<div></div>
		<div class="fork-top"></div>
	</div>
	<div class="row bottom-row" class:is-bottom={isBottom}>
		<div class="fork-bottom"></div>
		<div></div>
	</div>
</div>

<style lang="postcss">
	.fork {
		display: flex;
		flex-direction: column;
	}

	.row {
		display: grid;
		grid-template-columns: 1fr 1fr;
		flex-grow: 1;

		&.bottom-row {
			height: 16px;
			flex-grow: 0;
		}

		&.is-bottom {
			height: 8px;
		}
	}

	.fork-top {
		border-right: 2px var(--border-style) var(--border-color);
		border-bottom: 2px var(--border-style) var(--border-color);
		border-bottom-right-radius: 8px;
		margin-bottom: -2px;
	}

	.fork-bottom {
		border-left: 2px var(--border-style) var(--border-color);
		border-top: 2px var(--border-style) var(--border-color);
		border-top-left-radius: 8px;
	}
</style>
