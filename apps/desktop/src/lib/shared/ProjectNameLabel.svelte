<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	interface Props {
		projectName?: string;
	}

	let { projectName = '' }: Props = $props();
</script>

<div class="project-name">
	<Icon name="repo-book-small" />
	<span class="text-12 text-bold">
		{projectName}
	</span>
</div>

<style>
	.project-name {
		align-self: flex-start;
		display: flex;
		align-items: center;
		gap: 6px;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-50);
	}
</style>
