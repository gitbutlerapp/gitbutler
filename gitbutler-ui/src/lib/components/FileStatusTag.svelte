<script lang="ts">
	import Tag, { type TagColor } from './Tag.svelte';
	import type { FileStatus } from '$lib/utils/fileStatus';

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
