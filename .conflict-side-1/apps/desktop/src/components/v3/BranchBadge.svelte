<script lang="ts">
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { PushStatus } from '$lib/stacks/stack';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	type Props = {
		pushStatus: PushStatus;
	};

	const { pushStatus }: Props = $props();

	const [label, style] = $derived.by((): [string, ComponentColorType] => {
		switch (pushStatus) {
			case 'completelyUnpushed':
				return ['Unpushed', 'neutral'];
			case 'integrated':
				return ['Integrated', 'purple'];
			case 'unpushedCommits':
			case 'unpushedCommitsRequiringForce':
				return ['Diverged', 'warning'];
			case 'nothingToPush':
				return ['Pushed', 'pop'];
		}
	});
</script>

<Badge {style}>{label}</Badge>
