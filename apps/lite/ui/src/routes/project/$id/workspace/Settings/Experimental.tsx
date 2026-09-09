import { useSuspenseQueries } from "@tanstack/react-query";
import type { FC } from "react";
import { appSettingsQueryOptions, guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings, useUpdateFeatureFlags } from "#ui/api/mutations.ts";
import { Switch } from "#ui/components/Switch.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { Row, Section } from "./Section.tsx";

export const Experimental: FC = () => {
	const [{ data: settings }, { data: appSettings }] = useSuspenseQueries({
		queries: [guiSettingsQueryOptions, appSettingsQueryOptions],
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();
	const { mutate: updateFeatureFlags } = useUpdateFeatureFlags();

	return (
		<Section>
			<Row
				label="Comment annotations"
				labelId="comment-annotations"
				hint="Add comments to diff lines and copy them as feedback for an agent."
			>
				<Switch
					aria-labelledby="comment-annotations"
					checked={settings.commentAnnotations ?? defaultSettings.commentAnnotations}
					onCheckedChange={(commentAnnotations) => saveGUISettings({ commentAnnotations })}
				/>
			</Row>

			<Row
				label="Preview operations while dragging"
				labelId="dry-run-operations"
				hint="Dry-runs a drag-and-drop before it lands to show the outcome, such as conflicts. Slows dragging down."
			>
				<Switch
					aria-labelledby="dry-run-operations"
					checked={settings.dryRunOperations ?? defaultSettings.dryRunOperations}
					onCheckedChange={(dryRunOperations) => saveGUISettings({ dryRunOperations })}
				/>
			</Row>

			<Row
				label="Minimap"
				labelId="minimap"
				hint="A map of the diff down the right-hand edge, standing in for the scrollbar."
			>
				<Switch
					aria-labelledby="minimap"
					checked={settings.minimap ?? defaultSettings.minimap}
					onCheckedChange={(minimap) => saveGUISettings({ minimap })}
				/>
			</Row>

			<Row
				label="Linked worktrees"
				labelId="worktree-manipulation"
				hint="Shows linked git worktrees in the workspace. Existing ones start out archived; see the project's Worktrees page."
			>
				<Switch
					aria-labelledby="worktree-manipulation"
					checked={appSettings.featureFlags.worktreeManipulation}
					onCheckedChange={(worktreeManipulation) => updateFeatureFlags({ worktreeManipulation })}
				/>
			</Row>
		</Section>
	);
};
