<script lang="ts">
	import Icon from './Icon.svelte';
	export let projectName: string = '';
</script>

<div class="project-name">
	<Icon name="repo-book-small" />
	<span class="text-base-12 text-bold">
		{projectName}
	</span>
</div>

<style>
	.project-name {
		align-self: flex-start;
		display: flex;
		align-items: center;
		gap: var(--size-4);
		padding: var(--size-2) var(--size-4) var(--size-2) var(--size-2);
		border-radius: var(--radius-s);
		background: color-mix(in srgb, var(--clr-scale-ntrl-30) 8%, transparent);
		color: var(--clr-scale-ntrl-50);
	}
</style>
