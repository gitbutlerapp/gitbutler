<script lang="ts">
	import type { Color, Style } from '$lib/commitLines/types';

	interface Props {
		color: Color;
		style?: Style;
	}

	const { color, style }: Props = $props();
</script>

<div
	class="straight"
	class:none={color === 'none'}
	class:remote={color === 'remote'}
	class:local={color === 'local'}
	class:local-and-remote={color === 'localAndRemote'}
	class:shadow={color === 'shadow'}
	class:integrated={color === 'integrated'}
	class:dashed={style === 'dashed'}
></div>

<style lang="postcss">
	.straight {
		height: 100%;
		width: 100%;

		--border-color: black;

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
			border-style: dashed;
		}

		border-right: 2px solid var(--border-color);
	}
</style>
