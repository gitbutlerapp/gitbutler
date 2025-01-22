<script lang="ts">
	import type { Branch, Patch } from '@gitbutler/shared/branches/types';

	type Props = {
		branch: Branch;
	};

	const { branch }: Props = $props();

	const patches = branch.patches;

	function getClass(patch: Patch) {
		if (patch.reviewAll.rejected.length > 0) {
			return 'block rejected';
		}
		if (patch.reviewAll.signedOff.length > 0) {
			return 'block signoff';
		}
		return 'block';
	}
</script>

<div class="container">
	<p class="fact">{branch.stackSize}</p>
	<table class="graph" width="100%">
		<tbody>
			<tr>
				{#each patches as patch}
					<td class="patch"><div class={getClass(patch)}>&nbsp;</div></td>
				{/each}
			</tr>
		</tbody>
	</table>
</div>

<style lang="postcss">
	.container {
		display: flex;
		align-items: center;
		font-size: 0.8em;
		color: var(--clr-text-2);
	}

	.fact {
		margin-right: 8px;
	}

	.rejected {
		background-color: var(--clr-scale-err-50);
	}
	.signoff {
		background-color: var(--clr-scale-succ-50);
	}
	.discuss {
		background-color: var(--clr-scale-warn-50);
	}

	.block {
		margin-left: 0.5px;
	}
</style>
