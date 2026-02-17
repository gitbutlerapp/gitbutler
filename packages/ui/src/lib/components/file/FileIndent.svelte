<script lang="ts">
	interface Props {
		depth?: number;
	}

	const { depth }: Props = $props();
</script>

{#if depth && depth > 0}
	<div class="indent-wrapper">
		{#each Array(depth) as _}
			<div class="indent-wrapper__line"></div>
		{/each}
	</div>
{/if}

<style>
	.indent-wrapper {
		display: flex;
		align-items: center;
		height: 100%;
		gap: 6px;
		/* background-color: rgba(0, 0, 0, 0.1); */
	}

	.indent-wrapper__line {
		position: relative;
		width: 10px;
		height: 100%;
		/* background-color: rgba(0, 0, 0, 0.1); */

		&:before {
			position: absolute;
			top: 0;
			left: 50%;
			width: 1px;
			height: 100%;
			transform: translateX(-50%);
			background-color: var(--clr-text-2);
			content: "";
			opacity: 0.2;
		}
	}
</style>
