import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
import { USER_SERVICE } from "$lib/user/userService.svelte";
import { inject } from "@gitbutler/core/context";

/**
 * Returns the current user's profile picture if the given email belongs to them,
 * `undefined` otherwise. Matches against both the GitButler account email and the
 * local git config email, since these often differ.
 */
export function useUserAvatarUrl(): (email: string | null | undefined) => string | undefined {
	const userService = inject(USER_SERVICE);
	const gitConfigService = inject(GIT_CONFIG_SERVICE);

	let gitConfigEmail = $state<string | undefined>();

	$effect(() => {
		gitConfigService.get<string>("user.email").then((email) => {
			gitConfigEmail = email;
		});
	});

	return (email: string | null | undefined): string | undefined => {
		if (!email) return undefined;
		const user = userService.user;
		if (!user?.picture) return undefined;
		const lower = email.toLowerCase();
		if (lower === user.email?.toLowerCase() || lower === gitConfigEmail?.toLowerCase()) {
			return user.picture;
		}
		return undefined;
	};
}
