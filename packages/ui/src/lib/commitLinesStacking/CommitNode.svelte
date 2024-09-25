<script lang="ts">
	import type { CellType } from '$lib/commitLinesStacking/types';

	interface Props {
		type: CellType;
	}

	const { type }: Props = $props();

	const isSquircle = $derived(['Remote', 'Upstream', 'Integrated', 'LocalShadow'].includes(type));
</script>

<div
	class="container"
	class:remote={type === 'Remote'}
	class:local={type === 'Local'}
	class:local-shadow={type === 'LocalShadow'}
	class:upstream={type === 'Upstream'}
	class:integrated={type === 'Integrated'}
>
	<div class="commit-node-dot" class:squircle={isSquircle}></div>
	{#if type === 'LocalShadow'}
		<div class="commit-node-dot secondary"></div>
	{/if}
</div>

<style lang="postcss">
	.container {
		position: relative;
		z-index: var(--z-ground);

		&.remote {
			--border-color: var(--clr-commit-remote);
		}

		&.local {
			--border-color: var(--clr-commit-local);
		}

		&.local-shadow {
			--border-color: var(--clr-commit-local);
		}

		&.local-shadow .secondary {
			--border-color: var(--clr-commit-remote);
		}

		&.upstream {
			--border-color: var(--clr-commit-upstream);
		}

		&.integrated {
			--border-color: var(--clr-commit-integrated);
		}

		.commit-node-dot {
			height: 10px;
			width: 10px;
			margin-right: -4px;

			display: flex;
			align-items: center;
			justify-content: center;

			border-radius: 50%;
			background-color: var(--border-color);

			&.squircle {
				height: 0.625rem;
				width: 0.625rem;
				border-radius: 0.2rem;
				transform: rotate(45deg);
			}

			&.secondary {
				height: 0.7rem;
				width: 0.7rem;
				position: absolute;
				top: -0.025rem;
				left: -0.5rem;

				border-radius: 0.2rem;
				border: 0.175rem solid var(--clr-bg-2);
				transform: rotate(45deg);
			}
		}
	}
</style>
