<script lang="ts">
	import UpdateButton from '../UpdateButton.svelte';
	import type { Update } from '../../updater';
	import type { Observable } from 'rxjs';
	export let update: Observable<Update>;
</script>

<!-- App Updatesr -->
{#if $update?.enabled && $update?.shouldUpdate}
	<div class="border-color-4 flex-shrink-0 border-t px-4 py-4">
		<UpdateButton {update} />
	</div>
{/if}
