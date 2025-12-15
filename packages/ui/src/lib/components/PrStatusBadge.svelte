<script module lang="ts">
	export type PrStatusInfoType = 'loading' | 'open' | 'merged' | 'closed' | 'draft';
</script>

<script lang="ts">
	import Badge from '$components/Badge.svelte';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		status: PrStatusInfoType;
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
				return { text: 'Loading...', icon: 'spinner', style: 'gray' };
			case 'merged':
				return { text: 'Merged', icon: 'merged-pr-small', style: 'purple' };
			case 'closed':
				return { text: 'Closed', icon: 'closed-pr-small', style: 'error' };
			case 'draft':
				return { text: 'Draft', icon: 'draft-pr-small', style: 'gray' };
			default:
				return { text: 'Open', icon: 'pr-small', style: 'success' };
		}
	});
</script>

<Badge style={prStatusInfo.style} kind="soft" reversedDirection size="icon" icon={prStatusInfo.icon}
	>{prStatusInfo.text}</Badge
>
