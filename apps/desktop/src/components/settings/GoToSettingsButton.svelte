<script lang="ts">
	import { t } from "$lib/i18n";

	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { Button } from "@gitbutler/ui-svelte";

	const { openGeneralSettings } = useSettingsModal();
</script>

<Button
	style="pop"
	icon="login"
	onclick={async () => {
		openGeneralSettings("general");
	}}
>
	{t("go-to-settings")}
</Button>
