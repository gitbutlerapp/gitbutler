<script lang="ts">
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ProfileButton } from "@gitbutler/ui";

	const userService = inject(USER_SERVICE);
	const { openGeneralSettings } = useSettingsModal();
</script>

<ProfileButton
	srcUrl={userService.user?.picture ?? null}
	onclick={async () => openGeneralSettings()}
/>
