<script lang="ts">
	import Tag, { type TagStyle } from './Tag.svelte';
	import type { FileStatus } from '$lib/utils/fileStatus';

	export let status: FileStatus;

	function statusToColor(status: FileStatus): TagStyle {
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
