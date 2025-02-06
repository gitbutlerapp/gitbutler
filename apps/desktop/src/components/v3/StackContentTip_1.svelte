<script lang="ts">
	import CommitAndPush from '$lib/assets/illustrations/tip-commit-and-push.svg?raw';
	import { fade } from 'svelte/transition';
</script>

<div class="placeholder__svg" in:fade={{ duration: 250 }}>
	{@html CommitAndPush}
</div>
<div class="placeholder__text">
	<div class="text-18 text-semibold text-clr1">Commit and push</div>
	<div class="text-13 text-clr2">
		<p>
			File changes can be committed in any stack unless already committed in another, as they are
			dependent on a branch. When the dependent branch is selected, you can commit “locked” files.
			All branches in a stack are pushed together.
		</p>
	</div>
</div>

<style>
	.placeholder__svg {
		width: 300px;
	}

	.placeholder__text {
		text-align: left;
		line-height: 1 !important;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 8px;

		& p {
			line-height: 1.75;
		}
	}
</style>
