import { useSuspenseQueries } from "@tanstack/react-query";
import type { FC } from "react";
import { appSettingsQueryOptions, guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useRelaunch, useSaveGUISettings, useUpdateFeatureFlags } from "#ui/api/mutations.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Switch } from "@gitbutler/ui-react/Switch.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { Row, Section } from "./Section.tsx";

export const Experimental: FC = () => {
	const [{ data: settings }, { data: appSettings }] = useSuspenseQueries({
		queries: [guiSettingsQueryOptions, appSettingsQueryOptions],
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();
	const { mutate: updateFeatureFlags } = useUpdateFeatureFlags();
	const { mutate: relaunch, isPending: isRelaunching } = useRelaunch();

	return (
		<Section>
			<Row
				label="Comment annotations"
				labelId="comment-annotations"
				hint="Add comments to diff lines and copy them as feedback for an agent."
			>
				<Switch
					size="large"
					aria-labelledby="comment-annotations"
					checked={settings.commentAnnotations ?? defaultSettings.commentAnnotations}
					onCheckedChange={(commentAnnotations) => saveGUISettings({ commentAnnotations })}
				/>
			</Row>

			<Row
				label="Linked worktrees"
				labelId="worktree-manipulation"
				hint="Shows the repository's other worktrees in the workspace. Existing ones start out archived; bring them back under Project → Worktrees."
			>
				<Switch
					size="large"
					aria-labelledby="worktree-manipulation"
					checked={appSettings.featureFlags.worktreeManipulation}
					onCheckedChange={(worktreeManipulation) => updateFeatureFlags({ worktreeManipulation })}
				/>
			</Row>

			{window.lite.host === "tauri" && (
				<Row
					label="Lite interface"
					labelId="lite-frontend"
					hint="Turn off to go back to the classic interface. Takes effect after a restart."
					below={
						appSettings.featureFlags.liteFrontend ? undefined : (
							<Button disabled={isRelaunching} onClick={() => relaunch()}>
								Restart
							</Button>
						)
					}
				>
					<Switch
						size="large"
						aria-labelledby="lite-frontend"
						checked={appSettings.featureFlags.liteFrontend}
						onCheckedChange={(liteFrontend) => updateFeatureFlags({ liteFrontend })}
					/>
				</Row>
			)}
		</Section>
	);
};
