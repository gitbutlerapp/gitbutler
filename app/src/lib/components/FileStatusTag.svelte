<script lang="ts">
	import Tag from './Tag.svelte';
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

<Tag style={statusToColor(status)}>{status}</Tag>
