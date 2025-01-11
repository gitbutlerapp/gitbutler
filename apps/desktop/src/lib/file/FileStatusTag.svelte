<script lang="ts">
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { FileStatus } from '@gitbutler/ui/file/types';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	interface Props {
		status: FileStatus;
	}

	const { status }: Props = $props();

	function statusToColor(status: FileStatus): ComponentColorType {
		switch (status) {
			case 'A':
				return 'success';
			case 'M':
				return 'warning';
			case 'D':
				return 'error';
		}
	}
</script>

<Badge size="icon" style={statusToColor(status)}>{status}</Badge>
