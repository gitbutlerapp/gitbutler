import { useState } from "react";

export type ProjectRegistry<T> = (
	projectId: string,
) => [current: T, update: (projectId: string, f: (current: T) => T) => void];

export const useProjectRegistry = <T>(initial: T): ProjectRegistry<T> => {
	const [byProjectId, setByProjectId] = useState(() => new Map<string, T>());
	const updateProject = (projectId: string, update: (current: T) => T) =>
		setByProjectId((currm) => {
			const currv = currm.get(projectId) ?? initial;
			const nextv = update(currv);
			return nextv === currv ? currm : new Map(currm).set(projectId, nextv);
		});

	return (currentProjectId) => [byProjectId.get(currentProjectId) ?? initial, updateProject];
};
