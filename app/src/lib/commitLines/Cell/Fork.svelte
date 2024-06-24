<script lang="ts">
	import type { Color, Style } from '$lib/commitLines/types';

	interface Props {
		color: Color;
		style?: Style;
	}

	const { color, style }: Props = $props();
</script>

<div
	class="fork"
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
	<div class="row bottom-row">
		<div class="fork-bottom"></div>
		<div></div>
	</div>
</div>

<style lang="postcss">
	.fork {
		display: flex;
		flex-direction: column;

		width: 100%;
		height: 100%;

		--border-color: black;
		--border-style: solid;

		&.none {
			--border-color: transparent;
		}

		&.remote {
			--border-color: var(--clr-commit-upstream);
		}

		&.local {
			--border-color: var(--clr-commit-local);
		}

		&.local-and-remote {
			--border-color: var(--clr-commit-remote);
		}

		&.shadow {
			--border-color: var(--clr-commit-shadow);
		}

		&.integrated {
			--border-color: var(--clr-commit-shadow);
		}

		&.dashed {
			--border-style: dashed;
		}
	}

	.row {
		display: grid;
		grid-template-columns: 1fr 1fr;
		flex-grow: 1;

		&.bottom-row {
			height: 16px;
			flex-grow: 0;
		}
	}

	.fork-top {
		border-right: 2px solid var(--border-color);
		border-bottom: 2px solid var(--border-color);
		border-bottom-right-radius: 8px;
		margin-bottom: -2px;
	}

	.fork-bottom {
		border-left: 2px solid var(--border-color);
		border-top: 2px solid var(--border-color);
		border-top-left-radius: 8px;
	}
</style>
