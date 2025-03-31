<script lang="ts">
	interface Props {
		depth?: number;
	}

	const { depth }: Props = $props();
</script>

{#if depth && depth > 0}
	<div class="indent-wrapper">
		{#each Array(depth) as _}
			<div class="indent-wrapper__line" style="height: 100%;"></div>
		{/each}
	</div>
{/if}

<style>
	.indent-wrapper {
		display: flex;
		align-items: center;
		/* background-color: rgba(0, 0, 0, 0.1); */
		height: 100%;
		gap: 6px;
	}

	.indent-wrapper__line {
		position: relative;
		width: 10px;
		height: 100%;
		/* background-color: rgba(0, 0, 0, 0.1); */

		&:before {
			content: '';
			position: absolute;
			top: -50%;
			left: 50%;
			width: 1px;
			height: 200%;
			transform: translateX(-50%);
			background-color: var(--clr-border-1);
			opacity: 0.4;
			/* background-color: rgba(0, 0, 0, 0.4); */
		}
	}
</style>
