<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import type { CellType, CommitNodeData } from '$lib/commitLines/types';

	interface Props {
		commitNode: CommitNodeData;
		typeOverride?: CellType;
	}

	const { commitNode }: Props = $props();

	const tooltipText = $derived(commitNode.type ?? 'LocalOnly');
</script>

<div class="container">
	{#if commitNode.type === 'LocalOnly' && commitNode.commit?.remoteCommitId}
		<div class="local-shadow-commit-dot">
			<Tooltip text={commitNode.commit?.remoteCommitId.substring(0, 7) ?? 'Diverged'}>
				<svg class="shadow-dot" viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg">
					<path
						d="M0.827119 6.41372C0.0460709 5.63267 0.0460709 4.36634 0.827119 3.58529L3.70602 0.706392C4.48707 -0.0746567 5.7534 -0.0746567 6.53445 0.706392L9.41335 3.58529C10.1944 4.36634 10.1944 5.63267 9.41335 6.41372L6.53445 9.29262C5.7534 10.0737 4.48707 10.0737 3.70602 9.29262L0.827119 6.41372Z"
					/>
				</svg>
			</Tooltip>
			<Tooltip text="Diverged">
				<svg class="local-dot" viewBox="0 0 11 10" xmlns="http://www.w3.org/2000/svg">
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M0.740712 8.93256C1.59096 9.60118 2.66337 10 3.82893 10H5.82893C8.59035 10 10.8289 7.76142 10.8289 5C10.8289 2.23858 8.59035 0 5.82893 0H3.82893C2.66237 0 1.58912 0.399504 0.738525 1.06916L1.84289 2.17353C3.40499 3.73562 3.40499 6.26828 1.84289 7.83038L0.740712 8.93256Z"
					/>
				</svg>
			</Tooltip>
		</div>
	{:else if commitNode.type === 'LocalOnly'}
		<Tooltip text={tooltipText}>
			<svg
				class="local-commit-dot"
				viewBox="0 0 10 10"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<rect width="10" height="10" rx="5" />
			</svg>
		</Tooltip>
	{:else}
		<Tooltip text={tooltipText}>
			<svg class="generic-commit-dot" viewBox="0 0 11 12" xmlns="http://www.w3.org/2000/svg">
				<path
					d="M0.585786 7.41422C-0.195262 6.63317 -0.195262 5.36684 0.585786 4.58579L3.793 1.37857C4.57405 0.597523 5.84038 0.597524 6.62143 1.37857L9.82865 4.58579C10.6097 5.36684 10.6097 6.63317 9.82865 7.41422L6.62143 10.6214C5.84038 11.4025 4.57405 11.4025 3.793 10.6214L0.585786 7.41422Z"
				/>
			</svg>
		</Tooltip>
	{/if}
</div>

<style lang="postcss">
	.container {
		position: relative;
		z-index: var(--z-ground);
	}

	.local-commit-dot {
		width: 10px;
		height: 10px;
		transform: translateX(1px);
		fill: var(--clr-commit-local);
	}

	.generic-commit-dot {
		width: 11px;
		height: 12px;
		transform: translateX(2px);
		fill: var(--commit-color);
	}

	.local-shadow-commit-dot {
		display: flex;
		transform: translateX(5px);

		.shadow-dot {
			width: 10px;
			height: 10px;
			fill: var(--clr-commit-shadow);
		}

		.local-dot {
			width: 11px;
			height: 10px;
			fill: var(--clr-commit-local);
			transform: translateX(-1px);
		}
	}
</style>
