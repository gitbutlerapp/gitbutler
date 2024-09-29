<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import type { FileStatus } from '@gitbutler/ui/file/types';
	import type { ComponentColor } from '@gitbutler/ui/utils/colorTypes';

	interface Props {
		status: FileStatus;
	}

	let { status }: Props = $props();

	function statusToColor(status: FileStatus): ComponentColor {
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

<Button size="tag" clickable={false} style={statusToColor(status)}>{status}</Button>
