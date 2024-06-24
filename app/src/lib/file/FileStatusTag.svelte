<script lang="ts">
	import Button from '$lib/shared/Button.svelte';
	import type { FileStatus } from '$lib/utils/fileStatus';
	import type { ComponentColor } from '$lib/vbranches/types';

	export let status: FileStatus;

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
