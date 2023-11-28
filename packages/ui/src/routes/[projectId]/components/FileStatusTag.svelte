<script lang="ts">
	import type { FileStatus } from '$lib/vbranches/fileStatus';
	import Tag, { type TagColor } from './Tag.svelte';

	export let status: FileStatus;

	function statusToColor(status: FileStatus): TagColor {
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

<Tag color={statusToColor(status)} border>{status}</Tag>
