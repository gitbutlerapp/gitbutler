import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { appSettingsQueryOptions } from "#ui/api/queries.ts";
import { useUpdateMesh } from "#ui/api/mutations.ts";
import { Select } from "@gitbutler/ui-react/Select.tsx";
import { Switch } from "@gitbutler/ui-react/Switch.tsx";
import styles from "./General.module.css";
import { Row, Section } from "./Section.tsx";

const intervals = [5, 10, 30, 60, 120, 300, 600].map((seconds) => ({
	value: String(seconds),
	label:
		seconds < 60
			? `${seconds} seconds`
			: `${seconds / 60} ${seconds === 60 ? "minute" : "minutes"}`,
}));

/**
 * Following branches between machines, shared with `but mesh`. Which branches are followed is
 * chosen on each branch; these turn it on or off and pace it.
 */
export const Mesh: FC = () => {
	const { data: settings } = useSuspenseQuery(appSettingsQueryOptions);
	const { mutate: updateMesh } = useUpdateMesh();
	const { mesh } = settings;

	return (
		<>
			<Section>
				<Row
					label="Auto-publish"
					labelId="mesh-auto-publish"
					hint="Publish the branches you follow whenever they change, with a worktree's uncommitted changes."
				>
					<Switch
						size="large"
						aria-labelledby="mesh-auto-publish"
						checked={mesh.autoPublish}
						onCheckedChange={(autoPublish) => updateMesh({ autoPublish })}
					/>
				</Row>
				<Row label="Check for changes every" labelId="mesh-publish-interval">
					<Select
						aria-labelledby="mesh-publish-interval"
						className={styles.select}
						disabled={!mesh.autoPublish}
						items={intervals}
						value={String(mesh.publishIntervalSec)}
						onValueChange={(value) =>
							value !== null && updateMesh({ publishIntervalSec: Number(value) })
						}
					/>
				</Row>
			</Section>
			<Section>
				<Row
					label="Auto-pull"
					labelId="mesh-auto-pull"
					hint="Pull the branches you follow from other machines as they publish them. Local work that isn't published is never replaced."
				>
					<Switch
						size="large"
						aria-labelledby="mesh-auto-pull"
						checked={mesh.autoPull}
						onCheckedChange={(autoPull) => updateMesh({ autoPull })}
					/>
				</Row>
				<Row label="Pull a branch at most every" labelId="mesh-pull-interval">
					<Select
						aria-labelledby="mesh-pull-interval"
						className={styles.select}
						disabled={!mesh.autoPull}
						items={intervals}
						value={String(mesh.pullIntervalSec)}
						onValueChange={(value) =>
							value !== null && updateMesh({ pullIntervalSec: Number(value) })
						}
					/>
				</Row>
			</Section>
		</>
	);
};
