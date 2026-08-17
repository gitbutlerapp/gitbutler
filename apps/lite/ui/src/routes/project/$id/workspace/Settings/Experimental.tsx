import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { Switch } from "#ui/components/Switch.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { Row, Section } from "./Section.tsx";

export const Experimental: FC = () => {
	const { data: settings } = useSuspenseQuery(guiSettingsQueryOptions);
	const { mutate: saveGUISettings } = useSaveGUISettings();

	return (
		<Section>
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
		</Section>
	);
};
