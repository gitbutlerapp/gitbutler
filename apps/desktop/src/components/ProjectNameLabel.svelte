<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	interface Props {
		projectName?: string;
	}

	const { projectName = '' }: Props = $props();
</script>

<div class="project-name">
	<Icon name="repo-book-small" />
	<span class="text-13 text-semibold">
		{projectName}
	</span>
</div>

<style>
	.project-name {
		display: flex;
		align-items: center;
		align-self: flex-start;
		gap: 6px;
		border-radius: var(--radius-s);
		color: var(--clr-text-2);
	}
</style>
