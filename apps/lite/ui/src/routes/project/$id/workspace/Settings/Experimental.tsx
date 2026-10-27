import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { Switch } from "@gitbutler/ui-react/Switch.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { Row, Section } from "./Section.tsx";

export const Experimental: FC = () => {
	const { data: settings } = useSuspenseQuery(guiSettingsQueryOptions);
	const { mutate: saveGUISettings } = useSaveGUISettings();

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
		</Section>
	);
};
