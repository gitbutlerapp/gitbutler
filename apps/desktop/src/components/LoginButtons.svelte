<script lang="ts">
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { Button } from "@gitbutler/ui";

	const { openGeneralSettings } = useSettingsModal();
</script>

<Button
	style="pop"
	icon="signin"
	onclick={async () => {
		openGeneralSettings("general");
	}}
>
	Go to Settings
</Button>
