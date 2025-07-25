<script module lang="ts">
	export type StatusType = 'loading' | 'open' | 'merged' | 'closed' | 'draft';
</script>

<script lang="ts">
	import Badge from '$components/Badge.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	interface Props {
		status: StatusType;
	}

	type StatusInfo = {
		text: string;
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColorType;
	};

	const { status }: Props = $props();

	const prStatusInfo: StatusInfo = $derived.by(() => {
		switch (status) {
			case 'loading':
				return { text: 'Loading...', icon: 'spinner', style: 'neutral' };
			case 'merged':
				return { text: 'Merged', icon: 'merged-pr-small', style: 'purple' };
			case 'closed':
				return { text: 'Closed', icon: 'closed-pr-small', style: 'error' };
			case 'draft':
				return { text: 'Draft', icon: 'draft-pr-small', style: 'neutral' };
			default:
				return { text: 'Open', icon: 'pr-small', style: 'success' };
		}
	});
</script>

<Badge style={prStatusInfo.style} kind="soft" reversedDirection size="icon" icon={prStatusInfo.icon}
	>{prStatusInfo.text}</Badge
>
