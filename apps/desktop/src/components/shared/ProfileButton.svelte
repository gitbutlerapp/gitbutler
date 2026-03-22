<script lang="ts">
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { USER } from "$lib/user/user";
	import { inject } from "@gitbutler/core/context";
	import { ProfileButton } from "@gitbutler/ui";

	const user = inject(USER);
	const { openGeneralSettings } = useSettingsModal();
</script>

<ProfileButton srcUrl={$user?.picture ?? null} onclick={async () => openGeneralSettings()} />
