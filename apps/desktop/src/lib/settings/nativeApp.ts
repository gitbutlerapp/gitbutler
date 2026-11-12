import type { IBackend } from "$lib/backend";

/** Only the desktop app's nightly and release builds include the native app. */
export async function hasNativeApp(backend: IBackend): Promise<boolean> {
	if (backend.platformName === "web") return false;
	const info = await backend.getAppInfo();
	return ["GitButler", "GitButler Nightly"].includes(info.name);
}
