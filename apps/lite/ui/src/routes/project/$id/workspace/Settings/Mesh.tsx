import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { appSettingsQueryOptions } from "#ui/api/queries.ts";
import { useUpdateMesh } from "#ui/api/mutations.ts";
import { Select } from "@gitbutler/ui-react/Select.tsx";
import { Switch } from "@gitbutler/ui-react/Switch.tsx";
import styles from "./General.module.css";
import { Row, Section } from "./Section.tsx";

const durations = (seconds: Array<number>) =>
	seconds.map((s) => ({
		value: String(s),
		label:
			s < 60
				? `${s} ${s === 1 ? "second" : "seconds"}`
				: `${s / 60} ${s === 60 ? "minute" : "minutes"}`,
	}));
const settleTimes = durations([1, 2, 3, 5, 10, 30, 60]);

/** Keeping branches in step between machines, shared with `but mesh`. */
export const Mesh: FC = () => {
	const { data: settings } = useSuspenseQuery(appSettingsQueryOptions);
	const { mutate: updateMesh } = useUpdateMesh();
	const { mesh } = settings;

	return (
		<>
			<Section>
				<Row
					label="Keep published branches up to date"
					labelId="mesh-auto-publish"
					hint="Once you've published a branch, publish it again when its files or commits change. If you published a worktree's uncommitted changes, they're included."
				>
					<Switch
						size="large"
						aria-labelledby="mesh-auto-publish"
						checked={mesh.autoPublish}
						onCheckedChange={(autoPublish) => updateMesh({ autoPublish })}
					/>
				</Row>
				<Row label="Once changes settle for" labelId="mesh-publish-after">
					<Select
						aria-labelledby="mesh-publish-after"
						className={styles.select}
						disabled={!mesh.autoPublish}
						items={settleTimes}
						value={String(mesh.publishAfterSec)}
						onValueChange={(value) =>
							value !== null && updateMesh({ publishAfterSec: Number(value) })
						}
					/>
				</Row>
			</Section>
			<Section>
				<Row
					label="Pull branches sent to you"
					labelId="mesh-auto-pull"
					hint="When another machine sends you a branch, pull it into a worktree right away. Local work that isn't published is never replaced."
				>
					<Switch
						size="large"
						aria-labelledby="mesh-auto-pull"
						checked={mesh.autoPull}
						onCheckedChange={(autoPull) => updateMesh({ autoPull })}
					/>
				</Row>
			</Section>
		</>
	);
};
