<script lang="ts">
	import { getContext } from '$lib/context';
	import { PatchStacksService } from '$lib/stacks/service';

	/**
	 * Expects the following contexts:
	 * - PatchStacksService
	 */

	const _patchStacksService = getContext(PatchStacksService);
</script>
