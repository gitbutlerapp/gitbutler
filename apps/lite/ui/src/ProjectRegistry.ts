import { useState } from "react";

export type ProjectRegistry<T> = (
	projectId: string,
) => [current: T, update: (f: (current: T) => T) => void];

export const useProjectRegistry = <T>(initial: T): ProjectRegistry<T> => {
	const [byProjectId, setByProjectId] = useState(() => new Map<string, T>());

	return (projectId) => [
		byProjectId.get(projectId) ?? initial,
		(update) =>
			setByProjectId((currm) => {
				const currv = currm.get(projectId) ?? initial;
				const nextv = update(currv);
				return nextv === currv ? currm : new Map(currm).set(projectId, nextv);
			}),
	];
};
