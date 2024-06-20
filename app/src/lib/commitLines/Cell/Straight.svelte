<script lang="ts">
	import type { Style } from '$lib/commitLines/types';

	interface Props {
		style: Style;
	}

	const { style }: Props = $props();
</script>

<div
	class="straight"
	class:none={style === 'none'}
	class:remote={style === 'remote'}
	class:local={style === 'local'}
	class:local-dashed={style === 'localDashed'}
	class:local-and-remote={style === 'localAndRemote'}
	class:shadow={style === 'shadow'}
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

		&.local-dashed {
			--border-color: var(--clr-commit-local);
			border-style: dashed;
		}

		&.local-and-remote {
			--border-color: var(--clr-commit-remote);
		}

		&.shadow {
			--border-color: var(--clr-commit-shadow);
		}

		border-right: 2px solid var(--border-color);
	}
</style>
