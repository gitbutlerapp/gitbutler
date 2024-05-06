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
		gap: var(--size-6);
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-50);
	}
</style>
